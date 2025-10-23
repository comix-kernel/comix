//! constants for the operating system (platform-independent)
#![allow(unused)]

// about memory management
pub const PAGE_SIZE: usize = 4096;
pub const KERNEL_HEAP_SIZE: usize = 16 * 1024 * 1024; // 16MB
pub const USER_STACK_SIZE: usize = 4 * 1024 * 1024; // 4MB

// TODO: 这里之后应该改成从设备获取 CPU 数量
pub const NUM_CPU: usize = 4;

// memory layout constants
// temporarily set for QEMU RISC-V virt machine
// FIXME: refactor it to arch/riscv because it's platform-dependent
// TODO: fetch it form device tree in the future(after/while implemented devices feature)
pub const MEMORY_END: usize = 0x88000000; // 128MB for QEMU RISC-V virt

pub use crate::arch::platform::qemu::*;
