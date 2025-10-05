use super::core::AllocationInfo;

pub struct AllocGuard {
    name: &'static str,
    wrapper: bool,
}

impl AllocGuard {
    #[inline]
    pub fn new(name: &'static str, wrapper: bool) -> Self {
        // Start allocation tracking
        super::core::ALLOCATIONS.with(|stack| {
            let mut s = stack.borrow_mut();
            s.depth += 1;
            assert!((s.depth as usize) < super::core::MAX_DEPTH);
            let depth = s.depth as usize;
            s.elements[depth] = AllocationInfo::default();
        });

        Self { name, wrapper }
    }
}

impl Drop for AllocGuard {
    #[inline]
    fn drop(&mut self) {
        // Get allocation info and pop the frame
        let alloc_info = super::core::ALLOCATIONS.with(|stack| {
            let mut s = stack.borrow_mut();
            let depth = s.depth as usize;
            let popped = s.elements[depth];
            s.depth -= 1;
            let parent = s.depth as usize;
            s.elements[parent] += popped;
            popped
        });

        super::state::send_alloc_measurement(self.name, alloc_info, self.wrapper);
    }
}
