// constants for the operating system
#![allow(unused)]

// about memory management
pub const PAGE_SIZE: usize = 4096;
pub const KERNEL_HEAP_SIZE: usize = 16 * 1024 * 1024; // 16MB
pub const USER_STACK_SIZE: usize = 4 * 1024 * 1024; // 4MB

pub use crate::arch::riscv::platform::qemu::*;