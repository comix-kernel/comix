//! Global allocator module
//!
//! This module provides dynamic heap memory allocation functionality using the talc allocator.
//!
//! # Components
//!
//! - Global heap allocator based on talc::Talck
//! - Heap memory region defined by linker symbols
//! - Initialization function to set up the heap

use crate::{println, sync::RawSpinLockWithoutGuard};
use talc::{Span, Talc, Talck};

/// Global heap allocator instance
///
/// Uses talc's lock-based allocator (Talck) with our custom `RawSpinLockWithoutGuard`.
/// This lock implements `lock_api::RawMutex` and provides interrupt protection
/// to prevent deadlocks when interrupt handlers allocate memory.
///
/// Initialized with an empty span; actual memory will be claimed in init_heap().
#[global_allocator]
static ALLOCATOR: Talck<RawSpinLockWithoutGuard, talc::ClaimOnOom> =
    Talc::new(unsafe { talc::ClaimOnOom::new(Span::empty()) }).lock();

/// Initialize the heap allocator with the heap memory region defined in linker script
///
/// This function must be called early in the boot process, after BSS clearing
/// but before any heap allocations are attempted.
///
/// # Safety
///
/// - Must be called exactly once during boot
/// - Must be called before any heap allocations
/// - Heap region defined by linker symbols (sheap, eheap) must be valid
pub fn init_heap() {
    unsafe extern "C" {
        fn sheap();
        fn eheap();
    }

    let heap_start = sheap as usize;
    let heap_end = eheap as usize;
    let heap_size = heap_end - heap_start;

    println!(
        "Initializing heap: start={:#x}, end={:#x}, size={:#x} ({} MB)",
        heap_start,
        heap_end,
        heap_size,
        heap_size / 1024 / 1024
    );

    unsafe {
        ALLOCATOR
            .lock()
            .claim(Span::new(heap_start as *mut u8, heap_end as *mut u8))
            .expect("Failed to initialize heap allocator");
    }

    println!("Heap allocator initialized successfully");
}
