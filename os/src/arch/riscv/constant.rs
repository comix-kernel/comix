//! RISC-V 架构相关的常量定义
#![allow(unused)]

/// 架构名称字符串
pub const ARCH: &str = "riscv64";

/// riscv sstatus 寄存器中 SIE 位的掩码
pub const SSTATUS_SIE: usize = 1 << 1;
/// riscv sstatus 寄存器中 SPIE 位的掩码
pub const SSTATUS_SPIE: usize = 1 << 5;
/// riscv sstatus 寄存器中 SPP 位的掩码
pub const SSTATUS_SPP: usize = 1 << 8;

/// riscv sv39 地址空间布局常量
/// 包括用户空间和内核空间的地址范围
/// 用户空间地址范围: 0x0000_0000_0000_0000 - 0x0000_003f_ffff_ffff
/// 内核空间地址范围: 0xffff_ffc0_0000_0000 - 0xffff_ffff_ffff_ffff
pub const SV39_TOP_HALF_TOP: usize = 0xffff_ffff_ffff_ffff;
pub const SV39_TOP_HALF_BASE: usize = 0xffff_ffc0_0000_0000;
pub const SV39_BOT_HALF_TOP: usize = 0x0000_003f_ffff_ffff;
pub const SV39_BOT_HALF_BASE: usize = 0x0000_0000_0000_0000;

/// 16 字节对齐所需的掩码 (RV64)
pub const STACK_ALIGN_MASK: usize = 0xF;

/// 用户空间地址范围常量
pub const USER_BASE: usize = 0x0000_0000_0000_0000;
pub const USER_TOP: usize = 0x0000_003f_ffff_ffff;

pub const IRQ_MIN: usize = usize::MAX / 2;
pub const IRQ_MAX: usize = usize::MAX;

/// 软件中断（用于 IPI）
pub const SUPERVISOR_SOFTWARE: usize = usize::MAX / 2 + 1 + 1;
pub const TIMER: usize = usize::MAX / 2 + 1 + 5;
pub const SUPERVISOR_EXTERNAL: usize = usize::MAX / 2 + 1 + 8;
