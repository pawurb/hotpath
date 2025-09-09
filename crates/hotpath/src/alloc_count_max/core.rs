use std::cell::RefCell;

pub const MAX_DEPTH: usize = 64;

/// Allocation info tracking maximum count held at any point
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub struct AllocationInfo {
    /// The current (net result) number of allocations during a [measure()] call.
    pub count_current: i64,
    /// The max number of allocations held during a point in time during a [measure()] call.
    pub count_max: u64,
}

impl std::ops::AddAssign for AllocationInfo {
    fn add_assign(&mut self, other: Self) {
        self.count_current += other.count_current;
        self.count_max = self.count_max.max(other.count_max);
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
        let info = &mut stack.elements[depth];
        info.count_current += 1;
        if info.count_current > 0 {
            info.count_max = info.count_max.max(info.count_current as u64);
        }
    });
}

/// Called by the shared global allocator to track deallocations
#[inline]
pub fn track_dealloc() {
    ALLOCATIONS.with(|stack| {
        let mut stack = stack.borrow_mut();
        let depth = stack.depth as usize;
        let info = &mut stack.elements[depth];
        info.count_current -= 1;
    });
}
