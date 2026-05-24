//! 平台无关的常量
#![allow(dead_code)]

// about CPU and SMP
/// 最大支持的 CPU 核心数
pub const MAX_CPU_COUNT: usize = 8;

// about memory management
pub const PAGE_SIZE: usize = 4096;
pub const KERNEL_HEAP_SIZE: usize = 32 * 1024 * 1024; // 32MB(临时扩容, 原16MB)
pub const USER_STACK_SIZE: usize = 4 * 1024 * 1024; // 4MB

pub const MAX_ARGV: usize = 256;

use crate::arch::{ArchImpl, virtual_memory::VirtualMemory};
use crate::util::address::align_down;

// User space memory layout constants
// Memory layout (from high to low address):
// [USER_STACK]          <--
// ...
// [USER_HEAP]
// [USER_DATA]
// [USER_TEXT]

/// Leave one guard page below the top of the user address space.
pub const USER_STACK_TOP: usize = <ArchImpl as VirtualMemory>::USER_TOP - PAGE_SIZE;

/// Userspace rt_sigreturn trampoline address (one RX page).
///
/// Must not overlap with the user stack mapping. With the current (non page-aligned) `USER_STACK_TOP`,
/// the stack mapping ends at `align_down(USER_TOP, PAGE_SIZE)`, so we place the trampoline there.
pub const USER_SIGRETURN_TRAMPOLINE: usize =
    align_down(<ArchImpl as VirtualMemory>::USER_TOP, PAGE_SIZE);

/// Maximum heap size (prevent OOM)
pub const MAX_USER_HEAP_SIZE: usize = 64 * 1024 * 1024; // 64MB

pub const DEFAULT_MAX_FDS: usize = 256;

// Ext4 filesystem constants
/// Ext4 文件系统块大小 (必须与 mkfs.ext4 -b 参数一致)
pub const EXT4_BLOCK_SIZE: usize = 4096;
/// VirtIO 块设备扇区大小 (标准扇区大小)
pub const VIRTIO_BLK_SECTOR_SIZE: usize = 512;
/// 文件系统镜像大小 (与 qemu-run.sh 中的 fs.img 大小一致)
pub const FS_IMAGE_SIZE: usize = 1024 * 1024 * 1024; // 1 GB
