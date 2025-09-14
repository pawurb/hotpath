use std::cell::RefCell;

pub const MAX_DEPTH: usize = 64;

/// Minimal allocation info tracking only total bytes
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub struct AllocationInfo {
    /// The total amount of bytes allocated during a [measure()] call.
    pub bytes_total: u64,

    pub unsupported_async: bool,
}

impl std::ops::AddAssign for AllocationInfo {
    fn add_assign(&mut self, other: Self) {
        self.bytes_total += other.bytes_total;
        self.unsupported_async |= other.unsupported_async;
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
        // Only track if we're within a measured function (depth > 0) and within bounds
        if depth > 0 && depth < MAX_DEPTH {
            stack.elements[depth].bytes_total += size as u64;
        }
    });
}
