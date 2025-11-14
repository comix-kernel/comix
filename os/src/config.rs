//! 平台无关的常量
#![allow(unused)]

// about memory management
pub const PAGE_SIZE: usize = 4096;
pub const KERNEL_HEAP_SIZE: usize = 16 * 1024 * 1024; // 16MB
pub const USER_STACK_SIZE: usize = 4 * 1024 * 1024; // 4MB

pub const MAX_ARGV: usize = 256;

// User space memory layout constants
// Memory layout (from high to low address):
// [USER_STACK]          <--
// ...
// [USER_HEAP]
// [USER_DATA]
// [USER_TEXT]

pub const USER_STACK_TOP: usize = SV39_BOT_HALF_TOP - PAGE_SIZE; // leave another guard page

/// Maximum heap size (prevent OOM)
pub const MAX_USER_HEAP_SIZE: usize = 64 * 1024 * 1024; // 64MB

// TODO: 这里之后应该改成从设备获取 CPU 数量
pub const NUM_CPU: usize = 4;

// memory layout constants
// temporarily set for QEMU RISC-V virt machine
// FIXME: refactor it to arch/riscv because it's platform-dependent
// TODO: fetch it form device tree in the future(after/while implemented devices feature)
pub const MEMORY_END: usize = 0x88000000; // 128MB for QEMU RISC-V virt

pub const DEFAULT_MAX_FDS: usize = 256;

use crate::arch::constant::SV39_BOT_HALF_TOP;
pub use crate::arch::platform::qemu::*;
