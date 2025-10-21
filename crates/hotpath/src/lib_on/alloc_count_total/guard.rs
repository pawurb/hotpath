pub struct MeasurementGuard {
    name: &'static str,
    wrapper: bool,
    unsupported_async: bool,
}

impl MeasurementGuard {
    #[inline]
    pub fn new(name: &'static str, wrapper: bool, unsupported_async: bool) -> Self {
        if !unsupported_async {
            super::core::ALLOCATIONS.with(|stack| {
                let current_depth = stack.depth.get();
                stack.depth.set(current_depth + 1);
                assert!((stack.depth.get() as usize) < super::core::MAX_DEPTH);
                let depth = stack.depth.get() as usize;
                stack.elements[depth].count_total.set(0);
                stack.elements[depth].unsupported_async.set(false);
            });
        }

        Self {
            name,
            wrapper,
            unsupported_async,
        }
    }
}

impl Drop for MeasurementGuard {
    #[inline]
    fn drop(&mut self) {
        let (count_total, unsupported_async) = if self.unsupported_async {
            (0, true)
        } else {
            super::core::ALLOCATIONS.with(|stack| {
                let depth = stack.depth.get() as usize;
                let count = stack.elements[depth].count_total.get();
                let unsup_async = stack.elements[depth].unsupported_async.get();

                stack.depth.set(stack.depth.get() - 1);

                #[cfg(not(feature = "hotpath-alloc-self"))]
                {
                    let parent = stack.depth.get() as usize;
                    stack.elements[parent]
                        .count_total
                        .set(stack.elements[parent].count_total.get() + count);
                    stack.elements[parent]
                        .unsupported_async
                        .set(stack.elements[parent].unsupported_async.get() | unsup_async);
                }

                (count, unsup_async)
            })
        };

        super::state::send_alloc_measurement(
            self.name,
            count_total,
            unsupported_async,
            self.wrapper,
        );
    }
}
