use std::cell::RefCell;

pub const MAX_DEPTH: usize = 64;

/// Allocation info tracking maximum bytes held at any point
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub struct AllocationInfo {
    /// The current (net result) amount of bytes allocated during a [measure()] call.
    pub bytes_current: i64,
    /// The max amount of bytes allocated at one time during a [measure()] call.
    pub bytes_max: u64,
}

impl std::ops::AddAssign for AllocationInfo {
    fn add_assign(&mut self, other: Self) {
        self.bytes_current += other.bytes_current;
        self.bytes_max = self.bytes_max.max(other.bytes_max);
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
pub fn track_alloc(size: usize) {
    ALLOCATIONS.with(|stack| {
        let mut stack = stack.borrow_mut();
        let depth = stack.depth as usize;
        let info = &mut stack.elements[depth];
        info.bytes_current += size as i64;
        if info.bytes_current > 0 {
            info.bytes_max = info.bytes_max.max(info.bytes_current as u64);
        }
    });
}

/// Called by the shared global allocator to track deallocations
#[inline]
pub fn track_dealloc(size: usize) {
    ALLOCATIONS.with(|stack| {
        let mut stack = stack.borrow_mut();
        let depth = stack.depth as usize;
        let info = &mut stack.elements[depth];
        info.bytes_current -= size as i64;
    });
}
