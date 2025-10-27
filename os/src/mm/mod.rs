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
pub mod heap;
pub mod memory_space;
pub mod page_table;
pub mod physmem;

pub use frame_allocator::init_frame_allocator;
pub use global_allocator::init_heap;
pub use memory_space::memory_space::kernel_token;

use crate::config::{MEMORY_END, PAGE_SIZE};

unsafe extern "C" {
    fn ekernel();
}

/// Initializes the memory management subsystem
pub fn init() {
    // 1. Initialize frame allocator
    let start = (ekernel as usize).div_ceil(PAGE_SIZE) * PAGE_SIZE; // Page-aligned
    let end = MEMORY_END;

    println!(
        "[mm] Initializing frame allocator: {:#x} - {:#x}",
        start, end
    );
    init_frame_allocator(start, end);

    // 2. Initialize heap
    init_heap();
    println!("[mm] Heap initialized");

    // 3. Create and activate kernel address space (lazy_static will auto-initialize)
    let token = kernel_token();
    println!("[mm] Kernel page table token: {:#x}", token);

    // 4. Activate kernel page table
    #[cfg(target_arch = "riscv64")]
    unsafe {
        riscv::asm::sfence_vma_all();
        core::arch::asm!(
            "csrw satp, {satp}",
            satp = in(reg) token,
        );
        riscv::asm::sfence_vma_all();
    }

    println!("[mm] Memory management initialized successfully");
}
