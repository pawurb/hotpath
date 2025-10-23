pub struct MeasurementGuard {
    name: &'static str,
    wrapper: bool,
    unsupported_async: bool,
    thread_id: std::thread::ThreadId,
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
                stack.elements[depth].bytes_total.set(0);
                stack.elements[depth].unsupported_async.set(false);
            });
        }

        Self {
            name,
            wrapper,
            unsupported_async,
            thread_id: std::thread::current().id(),
        }
    }
}

impl Drop for MeasurementGuard {
    #[inline]
    fn drop(&mut self) {
        let cross_thread = std::thread::current().id() != self.thread_id;

        let (bytes_total, unsupported_async) = if self.unsupported_async || cross_thread {
            (0, self.unsupported_async)
        } else {
            super::core::ALLOCATIONS.with(|stack| {
                let depth = stack.depth.get() as usize;
                let bytes = stack.elements[depth].bytes_total.get();
                let unsup_async = stack.elements[depth].unsupported_async.get();

                stack.depth.set(stack.depth.get() - 1);

                #[cfg(not(feature = "hotpath-alloc-self"))]
                {
                    let parent = stack.depth.get() as usize;
                    stack.elements[parent]
                        .bytes_total
                        .set(stack.elements[parent].bytes_total.get() + bytes);
                    stack.elements[parent]
                        .unsupported_async
                        .set(stack.elements[parent].unsupported_async.get() | unsup_async);
                }

                (bytes, unsup_async)
            })
        };

        super::state::send_alloc_measurement(
            self.name,
            bytes_total,
            unsupported_async,
            self.wrapper,
            cross_thread,
        );
    }
}
