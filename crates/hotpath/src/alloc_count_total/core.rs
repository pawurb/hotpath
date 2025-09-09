use std::cell::RefCell;

pub const MAX_DEPTH: usize = 64;

/// Minimal allocation info tracking only total count
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub struct AllocationInfo {
    /// The total number of allocations made during a [measure()] call.
    pub count_total: u64,
}

impl std::ops::AddAssign for AllocationInfo {
    fn add_assign(&mut self, other: Self) {
        self.count_total += other.count_total;
    }
}

pub struct AllocationInfoStack {
    pub depth: u32,
    pub elements: [AllocationInfo; MAX_DEPTH],
}

thread_local! {
    pub static ALLOCATIONS: RefCell<AllocationInfoStack> = RefCell::new(AllocationInfoStack {
        depth: 0,
        elements: [AllocationInfo::default(); MAX_DEPTH],
    });
}

/// Called by the shared global allocator to track allocations
#[inline]
pub fn track_alloc() {
    ALLOCATIONS.with(|stack| {
        let mut stack = stack.borrow_mut();
        let depth = stack.depth as usize;
        stack.elements[depth].count_total += 1;
    });
}
