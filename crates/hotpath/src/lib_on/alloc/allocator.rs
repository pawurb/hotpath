// Original source: https://github.com/fornwall/allocation-counter
//
// Licensed under either of:
// - Apache License, Version 2.0.
// - MIT/X Consortium License
//
// Modifications:
// - Adjusted to work with hotpath module system
// - Split into feature-specific dispatching allocator

use std::alloc::{GlobalAlloc, Layout, System};

/// Shared global allocator that dispatches to enabled allocation tracking features
pub struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        #[cfg(feature = "hotpath-alloc-bytes-total")]
        crate::lib_on::alloc_bytes_total::core::track_alloc(layout.size());

        #[cfg(feature = "hotpath-alloc-count-total")]
        crate::lib_on::alloc_count_total::core::track_alloc();

        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe {
            System.dealloc(ptr, layout);
        }
    }
}
