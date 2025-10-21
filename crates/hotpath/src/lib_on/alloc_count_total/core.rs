use std::cell::Cell;

pub const MAX_DEPTH: usize = 64;

/// Minimal allocation info tracking only total count
pub struct AllocationInfo {
    /// The total number of allocations made during a [measure()] call.
    pub count_total: Cell<u64>,

    pub unsupported_async: Cell<bool>,
}

impl std::ops::AddAssign for AllocationInfo {
    fn add_assign(&mut self, other: Self) {
        self.count_total
            .set(self.count_total.get() + other.count_total.get());
        self.unsupported_async
            .set(self.unsupported_async.get() | other.unsupported_async.get());
    }
}

pub struct AllocationInfoStack {
    pub depth: Cell<u32>,
    pub elements: [AllocationInfo; MAX_DEPTH],
}

thread_local! {
    pub static ALLOCATIONS: AllocationInfoStack = const { AllocationInfoStack {
        depth: Cell::new(0),
        elements: [const { AllocationInfo { count_total: Cell::new(0), unsupported_async: Cell::new(false) } }; MAX_DEPTH],
    } };
}

/// Called by the shared global allocator to track allocations
#[inline]
pub fn track_alloc() {
    ALLOCATIONS.with(|stack| {
        let depth = stack.depth.get() as usize;
        let info = &stack.elements[depth];
        info.count_total.set(info.count_total.get() + 1);
    });
}
