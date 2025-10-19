use super::core::AllocationInfo;

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
                let mut s = stack.borrow_mut();
                s.depth += 1;
                assert!((s.depth as usize) < super::core::MAX_DEPTH);
                let depth = s.depth as usize;
                s.elements[depth] = AllocationInfo::default();
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
        // Get allocation info and pop the frame
        let alloc_info = if self.unsupported_async {
            crate::lib_on::alloc_count_total::core::AllocationInfo {
                count_total: 0,
                unsupported_async: true,
            }
        } else {
            super::core::ALLOCATIONS.with(|stack| {
                let mut s = stack.borrow_mut();
                let depth = s.depth as usize;
                let popped = s.elements[depth];
                s.depth -= 1;
                #[cfg(not(feature = "hotpath-alloc-self"))]
                {
                    let parent = s.depth as usize;
                    s.elements[parent] += popped;
                }
                popped
            })
        };

        super::state::send_alloc_measurement(self.name, alloc_info, self.wrapper);
    }
}
