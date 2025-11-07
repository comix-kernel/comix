//! constants for the operating system (platform-independent)
#![allow(unused)]

// about memory management
pub const PAGE_SIZE: usize = 4096;
pub const KERNEL_HEAP_SIZE: usize = 16 * 1024 * 1024; // 16MB
pub const USER_STACK_SIZE: usize = 4 * 1024 * 1024; // 4MB

// User space memory layout constants
// Memory layout (from high to low address):
// [TRAMPOLINE]          <-- usize::MAX - PAGE_SIZE + 1
// [GUARD_PAGE]          <-- unmapped guard page
// [TRAP_CONTEXT]        <-- TRAMPOLINE - 2 * PAGE_SIZE
// [GUARD_PAGE]          <-- unmapped guard page
// [USER_STACK]          <-- TRAP_CONTEXT - 2 * PAGE_SIZE - USER_STACK_SIZE
// ...
// [USER_HEAP]
// [USER_DATA]
// [USER_TEXT]

pub const TRAMPOLINE: usize = SV39_BOT_HALF_TOP - PAGE_SIZE + 1;
pub const TRAP_CONTEXT: usize = TRAMPOLINE - 2 * PAGE_SIZE; // leave one guard page
pub const USER_STACK_TOP: usize = TRAP_CONTEXT - PAGE_SIZE; // leave another guard page

/// Maximum heap size (prevent OOM)
pub const MAX_USER_HEAP_SIZE: usize = 64 * 1024 * 1024; // 64MB

// TODO: 这里之后应该改成从设备获取 CPU 数量
pub const NUM_CPU: usize = 4;

// memory layout constants
// temporarily set for QEMU RISC-V virt machine
// FIXME: refactor it to arch/riscv because it's platform-dependent
// TODO: fetch it form device tree in the future(after/while implemented devices feature)
pub const MEMORY_END: usize = 0x88000000; // 128MB for QEMU RISC-V virt

use crate::arch::constant::SV39_BOT_HALF_TOP;
pub use crate::arch::platform::qemu::*;
