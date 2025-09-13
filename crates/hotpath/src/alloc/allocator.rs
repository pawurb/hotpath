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

thread_local! {
    pub static DO_COUNT: std::cell::RefCell<u32> = const { std::cell::RefCell::new(0) };
}

/// Shared global allocator that dispatches to enabled allocation tracking features
pub struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        DO_COUNT.with(|do_count| {
            if *do_count.borrow() == 0 {
                #[cfg(feature = "hotpath-alloc-bytes-total")]
                crate::alloc_bytes_total::core::track_alloc(layout.size());

                #[cfg(feature = "hotpath-alloc-bytes-max")]
                crate::alloc_bytes_max::core::track_alloc(layout.size());

                #[cfg(feature = "hotpath-alloc-count-total")]
                crate::alloc_count_total::core::track_alloc();

                #[cfg(feature = "hotpath-alloc-count-max")]
                crate::alloc_count_max::core::track_alloc();
            }
        });

        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        DO_COUNT.with(|do_count| {
            if *do_count.borrow() == 0 {
                // Dispatch to enabled allocation tracking features
                #[cfg(feature = "hotpath-alloc-bytes-max")]
                crate::alloc_bytes_max::core::track_dealloc(layout.size());

                #[cfg(feature = "hotpath-alloc-count-max")]
                crate::alloc_count_max::core::track_dealloc();
            }
        });

        unsafe {
            System.dealloc(ptr, layout);
        }
    }
}

/// Re-export the allocator for easy access as alloc::core::allocator
pub use CountingAllocator as allocator;
