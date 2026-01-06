//! 平台无关的常量
#![allow(unused)]

// about CPU and SMP
/// 最大支持的 CPU 核心数
pub const MAX_CPU_COUNT: usize = 8;

// about memory management
pub const PAGE_SIZE: usize = 4096;
pub const KERNEL_HEAP_SIZE: usize = 32 * 1024 * 1024; // 32MB(临时扩容, 原16MB)
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

// memory layout constants
#[cfg(target_arch = "riscv64")]
pub const MEMORY_END: usize = 0x8800_0000; // 128MB for QEMU RISC-V virt
#[cfg(target_arch = "loongarch64")]
pub const MEMORY_END: usize = crate::arch::platform::virt::MEMORY_END;

pub const DEFAULT_MAX_FDS: usize = 256;

// Ext4 filesystem constants
/// Ext4 文件系统块大小 (必须与 mkfs.ext4 -b 参数一致)
pub const EXT4_BLOCK_SIZE: usize = 4096;
/// VirtIO 块设备扇区大小 (标准扇区大小)
pub const VIRTIO_BLK_SECTOR_SIZE: usize = 512;
/// 文件系统镜像大小 (与 qemu-run.sh 中的 fs.img 大小一致)
pub const FS_IMAGE_SIZE: usize = 1024 * 1024 * 1024; // 1 GB

use crate::arch::constant::SV39_BOT_HALF_TOP;
pub use crate::arch::platform::virt::*;
