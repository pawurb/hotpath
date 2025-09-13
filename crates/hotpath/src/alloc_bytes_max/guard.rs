use crate::alloc_bytes_max::core::AllocationInfo;

pub struct AllocGuard {
    name: &'static str,
}

impl AllocGuard {
    #[inline]
    pub fn new(name: &'static str) -> Self {
        // Start allocation tracking
        crate::alloc_bytes_max::core::ALLOCATIONS.with(|stack| {
            let mut s = stack.borrow_mut();
            s.depth += 1;
            assert!((s.depth as usize) < crate::alloc_bytes_max::core::MAX_DEPTH);
            let depth = s.depth as usize;
            s.elements[depth] = AllocationInfo::default();
        });

        Self { name }
    }
}

impl Drop for AllocGuard {
    #[inline]
    fn drop(&mut self) {
        // Get allocation info and pop the frame
        let alloc_info = crate::alloc_bytes_max::core::ALLOCATIONS.with(|stack| {
            let mut s = stack.borrow_mut();
            let depth = s.depth as usize;
            let popped = s.elements[depth];
            s.depth -= 1;
            let parent = s.depth as usize;
            s.elements[parent] += popped;
            popped
        });

        crate::alloc_bytes_max::state::send_alloc_measurement(self.name, alloc_info);
    }
}
