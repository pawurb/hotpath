//! Thread-local stack of instrumented-function names used to attribute SQL
//! queries and HTTP requests to the innermost measured function (the "Source"
//! column). Sync measurement guards push on creation and pop on drop; async
//! bodies push and pop around every poll (see `futures::wrapper`), so tasks
//! interleaved on one runtime thread never observe a stale caller.
//!
//! Compiles to no-ops unless a SQL or HTTP front-end feature is enabled.
//!
//! Also holds the per-thread axum route context (the "Route" column): the
//! server middleware enters the matched route template around every poll of
//! the handler future, and SQL/HTTP front-ends read it alongside the caller.

cfg_if::cfg_if! {
    if #[cfg(any(
        feature = "sqlx",
        feature = "diesel",
        feature = "toasty",
        feature = "reqwest-0-12",
        feature = "reqwest-0-13",
        feature = "ureq-3",
    ))] {
        use std::cell::Cell;

        const MAX_DEPTH: usize = 64;

        struct CallerStack {
            depth: Cell<usize>,
            names: [Cell<&'static str>; MAX_DEPTH],
        }

        thread_local! {
            static CALLER_STACK: CallerStack = const {
                CallerStack {
                    depth: Cell::new(0),
                    names: [const { Cell::new("") }; MAX_DEPTH],
                }
            };
        }

        /// Pushes beyond `MAX_DEPTH` only bump the depth counter so pops stay
        /// balanced; `current_caller` then reports the deepest recorded name.
        #[inline]
        pub(crate) fn push_caller(name: &'static str) {
            let _ = CALLER_STACK.try_with(|stack| {
                let depth = stack.depth.get();
                if depth < MAX_DEPTH {
                    stack.names[depth].set(name);
                }
                stack.depth.set(depth + 1);
            });
        }

        #[inline]
        pub(crate) fn pop_caller() {
            let _ = CALLER_STACK.try_with(|stack| {
                let depth = stack.depth.get();
                debug_assert!(depth > 0, "pop_caller called with depth 0");
                if depth > 0 {
                    stack.depth.set(depth - 1);
                }
            });
        }

        #[inline]
        pub(crate) fn current_caller() -> Option<&'static str> {
            CALLER_STACK
                .try_with(|stack| {
                    let depth = stack.depth.get();
                    if depth == 0 {
                        None
                    } else {
                        Some(stack.names[depth.min(MAX_DEPTH) - 1].get())
                    }
                })
                .ok()
                .flatten()
        }
    } else {
        #[inline]
        pub(crate) fn push_caller(_name: &'static str) {}

        #[inline]
        pub(crate) fn pop_caller() {}

        #[inline]
        #[allow(dead_code)]
        pub(crate) fn current_caller() -> Option<&'static str> {
            None
        }
    }
}

/// Number of SQL queries and outbound HTTP requests one server request has
/// issued, carried by [`crate::lib_on::server::ServerEvent::Completed`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RequestCalls {
    pub(crate) sql: u32,
    pub(crate) http: u32,
}

impl RequestCalls {
    #[allow(dead_code)]
    pub(crate) const ZERO: Self = Self { sql: 0, http: 0 };
}

/// Bytes and allocations one server request has made under its route scope,
/// carried by [`crate::lib_on::server::ServerEvent::Completed`]. Only the
/// `hotpath-alloc` allocator fills it; other builds carry zeros that the
/// report never shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RequestAlloc {
    pub(crate) bytes: u64,
    pub(crate) count: u64,
}

impl RequestAlloc {
    #[allow(dead_code)]
    pub(crate) const ZERO: Self = Self { bytes: 0, count: 0 };
}

cfg_if::cfg_if! {
    if #[cfg(feature = "axum-0-8")] {
        use std::collections::HashSet;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Once, RwLock};

        thread_local! {
            static CURRENT_ROUTE: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
            /// SQL queries and outbound HTTP requests issued so far by the
            /// request currently in scope; installed by [`enter_route`] and
            /// handed back to the request when its scope drops.
            static REQUEST_CALLS: std::cell::Cell<RequestCalls> = const { std::cell::Cell::new(RequestCalls::ZERO) };
        }


        static ROUTE_SCOPE_ENABLED: AtomicBool = AtomicBool::new(true);

        static INTERNED_ROUTES: RwLock<Option<HashSet<&'static str>>> = RwLock::new(None);

        static NESTED_SCOPE_WARNING: Once = Once::new();

        /// Disables or enables attributing SQL queries and HTTP requests to
        /// the axum route that triggered them.
        pub(crate) fn set_route_scope(enabled: bool) {
            ROUTE_SCOPE_ENABLED.store(enabled, Ordering::Relaxed);
        }

        pub(crate) fn route_scope_enabled() -> bool {
            ROUTE_SCOPE_ENABLED.load(Ordering::Relaxed)
        }

        /// Leaks each distinct route template once so the thread-local stays
        /// `Copy`. The set is capped at `HOTPATH_ENTRIES_LIMIT` like the
        /// per-subsystem maps; templates beyond the cap get no route context.
        pub(crate) fn intern_route(route: &str) -> Option<&'static str> {
            if let Some(found) = INTERNED_ROUTES
                .read()
                .unwrap()
                .as_ref()
                .and_then(|set| set.get(route).copied())
            {
                return Some(found);
            }
            let _suspend = crate::lib_on::SuspendAllocTracking::new();
            let mut guard = INTERNED_ROUTES.write().unwrap();
            let set = guard.get_or_insert_with(HashSet::new);
            if let Some(found) = set.get(route) {
                return Some(found);
            }
            let limit = *crate::lib_on::hotpath_guard::ENTRIES_LIMIT;
            if limit > 0 && set.len() >= limit {
                return None;
            }
            let leaked: &'static str = Box::leak(route.to_owned().into_boxed_str());
            set.insert(leaked);
            Some(leaked)
        }

        /// Sets the current route for the duration of the returned guard and
        /// clears it on drop.
        ///
        /// Scopes are exclusive per thread: polls are synchronous, so a route
        /// already set means the same request is passing through `AxumLayer`
        /// twice (nested router wrapped separately, or a sub-request into a
        /// wrapped router). The inner entry returns `None` and warns once; the
        /// outer scope keeps the request's counters.
        ///
        /// `calls` and `alloc` hold the request's running SQL / HTTP counts and
        /// allocation totals: installed for the scope and written back when
        /// the guard drops, so the owner sees the totals across polls.
        #[inline]
        pub(crate) fn enter_route<'a>(
            route: &'static str,
            calls: &'a mut RequestCalls,
            alloc: &'a mut RequestAlloc,
        ) -> Option<RouteScopeGuard<'a>> {
            let entered = CURRENT_ROUTE
                .try_with(|cell| match cell.get() {
                    Some(existing) => Err(existing),
                    None => {
                        cell.set(Some(route));
                        Ok(())
                    }
                })
                .unwrap_or(Ok(()));
            if let Err(existing) = entered {
                warn_nested_scope(existing, route);
                return None;
            }
            let _ = REQUEST_CALLS.try_with(|cell| cell.set(*calls));
            #[cfg(feature = "hotpath-alloc")]
            let entry_depth = crate::functions::alloc::core::route_alloc_enter(*alloc);
            Some(RouteScopeGuard {
                calls,
                alloc,
                #[cfg(feature = "hotpath-alloc")]
                entry_depth,
            })
        }

        #[cold]
        fn warn_nested_scope(existing: &'static str, route: &'static str) {
            NESTED_SCOPE_WARNING.call_once(|| {
                let _suspend = crate::lib_on::SuspendAllocTracking::new();
                eprintln!(
                    "hotpath: route scope for `{existing}` is still active while entering \
                     `{route}`; apply `AxumLayer` once per Router (nested routers and \
                     sub-requests are reported by the outer layer only)"
                );
            });
        }

        #[inline]
        #[allow(dead_code)]
        pub(crate) fn current_route() -> Option<&'static str> {
            CURRENT_ROUTE.try_with(|cell| cell.get()).ok().flatten()
        }

        /// Route of the request issuing a SQL query, counting the query
        /// towards that request's `SQL/req`.
        #[inline]
        #[allow(dead_code)]
        pub(crate) fn current_sql_route() -> Option<&'static str> {
            let route = current_route()?;
            let _ = REQUEST_CALLS.try_with(|cell| {
                let mut calls = cell.get();
                calls.sql = calls.sql.saturating_add(1);
                cell.set(calls);
            });
            Some(route)
        }

        /// Route of the request issuing an outbound HTTP request, counting
        /// it towards that request's `HTTP/req`.
        #[inline]
        #[allow(dead_code)]
        pub(crate) fn current_http_route() -> Option<&'static str> {
            let route = current_route()?;
            let _ = REQUEST_CALLS.try_with(|cell| {
                let mut calls = cell.get();
                calls.http = calls.http.saturating_add(1);
                cell.set(calls);
            });
            Some(route)
        }

        pub(crate) struct RouteScopeGuard<'a> {
            calls: &'a mut RequestCalls,
            #[cfg_attr(not(feature = "hotpath-alloc"), allow(dead_code))]
            alloc: &'a mut RequestAlloc,
            #[cfg(feature = "hotpath-alloc")]
            entry_depth: u32,
        }

        impl Drop for RouteScopeGuard<'_> {
            #[inline]
            fn drop(&mut self) {
                #[cfg(feature = "hotpath-alloc")]
                {
                    *self.alloc = crate::functions::alloc::core::route_alloc_exit(self.entry_depth);
                }
                let _ = CURRENT_ROUTE.try_with(|cell| cell.set(None));
                let _ = REQUEST_CALLS.try_with(|cell| {
                    *self.calls = cell.replace(RequestCalls::ZERO);
                });
            }
        }
    } else {
        #[inline]
        pub(crate) fn current_route() -> Option<&'static str> {
            None
        }

        #[inline]
        #[allow(dead_code)]
        pub(crate) fn current_sql_route() -> Option<&'static str> {
            None
        }

        #[inline]
        #[allow(dead_code)]
        pub(crate) fn current_http_route() -> Option<&'static str> {
            None
        }
    }
}

#[cfg(all(test, feature = "axum-0-8"))]
mod tests {
    use crate::lib_on::caller_stack::{
        current_http_route, current_route, current_sql_route, enter_route, intern_route,
        RequestAlloc, RequestCalls,
    };

    #[test]
    fn route_scope_counts_calls_and_is_exclusive() {
        assert_eq!(current_route(), None);
        let outer = intern_route("GET /outer").unwrap();
        let inner = intern_route("GET /inner").unwrap();
        assert!(std::ptr::eq(outer, intern_route("GET /outer").unwrap()));
        let mut outer_calls = RequestCalls::ZERO;
        let mut outer_alloc = RequestAlloc::ZERO;
        let mut inner_calls = RequestCalls::ZERO;
        let mut inner_alloc = RequestAlloc::ZERO;
        {
            let _outer =
                enter_route(outer, &mut outer_calls, &mut outer_alloc).expect("first scope enters");
            assert_eq!(current_route(), Some(outer));
            assert_eq!(current_sql_route(), Some(outer));
            {
                // The same request through a second layer: no scope, the
                // outer one keeps counting.
                let nested = enter_route(inner, &mut inner_calls, &mut inner_alloc);
                assert!(nested.is_none());
                assert_eq!(current_route(), Some(outer));
                assert_eq!(current_sql_route(), Some(outer));
                assert_eq!(current_http_route(), Some(outer));
            }
            assert_eq!(current_route(), Some(outer));
        }
        assert_eq!(current_route(), None);
        // Calls outside any scope are not counted.
        assert_eq!(current_sql_route(), None);
        assert_eq!(outer_calls, RequestCalls { sql: 2, http: 1 });
        assert_eq!(inner_calls, RequestCalls::ZERO);

        // Re-entering resumes the previous counts, as across polls.
        {
            let _outer = enter_route(outer, &mut outer_calls, &mut outer_alloc).unwrap();
            current_sql_route();
        }
        assert_eq!(outer_calls, RequestCalls { sql: 3, http: 1 });

        // A different request enters once the previous scope is gone.
        {
            let _inner = enter_route(inner, &mut inner_calls, &mut inner_alloc).unwrap();
            assert_eq!(current_http_route(), Some(inner));
        }
        assert_eq!(inner_calls, RequestCalls { sql: 0, http: 1 });
    }

    // No counting allocator in lib tests, so feed `track_alloc` directly.
    #[cfg(feature = "hotpath-alloc")]
    #[test]
    fn route_scope_accumulates_allocations_across_polls() {
        use crate::functions::alloc::core::track_alloc;
        use crate::functions::alloc::guard::{pop_alloc_stack, push_alloc_stack};

        let route = intern_route("GET /alloc").unwrap();
        let mut calls = RequestCalls::ZERO;
        let mut alloc = RequestAlloc::ZERO;
        {
            let _scope = enter_route(route, &mut calls, &mut alloc).unwrap();
            // Allocated directly under the route frame.
            track_alloc(4096);
            // Allocated inside a measured function: its exclusive frame is
            // folded into the request when it pops.
            push_alloc_stack();
            track_alloc(2048);
            assert_eq!(pop_alloc_stack(), (2048, 1));
        }
        assert_eq!(
            alloc,
            RequestAlloc {
                bytes: 6144,
                count: 2
            }
        );

        // A second poll of the same request keeps adding to the carried total.
        {
            let _scope = enter_route(route, &mut calls, &mut alloc).unwrap();
            track_alloc(1024);
        }
        assert_eq!(
            alloc,
            RequestAlloc {
                bytes: 7168,
                count: 3
            }
        );

        // Allocations outside any scope stay out, and a measured frame popped
        // outside a scope is not attributed to the last request.
        track_alloc(8192);
        push_alloc_stack();
        track_alloc(16);
        pop_alloc_stack();
        assert_eq!(
            alloc,
            RequestAlloc {
                bytes: 7168,
                count: 3
            }
        );

        // A new request starts from its own carried total.
        let mut other = RequestAlloc::ZERO;
        {
            let _scope = enter_route(route, &mut calls, &mut other).unwrap();
            track_alloc(1);
        }
        assert_eq!(other, RequestAlloc { bytes: 1, count: 1 });
        assert_eq!(
            alloc,
            RequestAlloc {
                bytes: 7168,
                count: 3
            }
        );

        // A measured function enclosing the scope (middleware outside the
        // layer) still sees the route frame's residual as its own bytes; a
        // measured child inside the scope stays exclusive.
        let mut enclosed = RequestAlloc::ZERO;
        push_alloc_stack();
        {
            let _scope = enter_route(route, &mut calls, &mut enclosed).unwrap();
            track_alloc(512);
            push_alloc_stack();
            track_alloc(64);
            pop_alloc_stack();
        }
        assert_eq!(pop_alloc_stack(), (512, 1));
        assert_eq!(
            enclosed,
            RequestAlloc {
                bytes: 576,
                count: 2
            }
        );
    }
}
