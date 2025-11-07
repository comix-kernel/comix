#![allow(dead_code)]
//! Memory management module
//!
//! This module provides architecture-independent memory management abstractions
//! and implementations for the kernel.
//!
//! # Components
//!
//! - [`address`]: Address and page number abstractions
//! - [`frame_allocator`]: Physical frame allocation
//! - [`global_allocator`]: Global heap allocator
//! - [`memory_space`]: Memory space management
//! - [`page_table`]: Page table abstractions and implementations(arch-independent)

pub mod address;
pub mod frame_allocator;
pub mod global_allocator;
pub mod memory_space;
pub mod page_table;

pub use frame_allocator::init_frame_allocator;
pub use global_allocator::init_heap;

use crate::arch::mm::vaddr_to_paddr;
use crate::config::{MEMORY_END, PAGE_SIZE};
use crate::mm::address::Ppn;
use crate::mm::memory_space::with_kernel_space;

unsafe extern "C" {
    fn ekernel();
}

/// Initializes the memory management subsystem
pub fn init() {
    // 1. Initialize frame allocator
    // ekernel is a virtual address, need to convert to physical address
    let ekernel_paddr = unsafe { vaddr_to_paddr(ekernel as usize) };
    let start = ekernel_paddr.div_ceil(PAGE_SIZE) * PAGE_SIZE; // Page-aligned
    let end = MEMORY_END;

    init_frame_allocator(start, end);

    // 2. Initialize heap
    init_heap();

    // 3. Create and activate kernel address space (lazy_static will auto-initialize)
    #[cfg(target_arch = "riscv64")]
    {
        let root_ppn = with_kernel_space(|space| space.root_ppn());
        activate(root_ppn);
    }
}

pub fn activate(root_ppn: Ppn) {
    use crate::mm::page_table::PageTableInner as PageTableInnerTrait;
    crate::arch::mm::PageTableInner::activate(root_ppn);
}
