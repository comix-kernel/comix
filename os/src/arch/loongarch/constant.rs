//! LoongArch64 架构常量定义
#![allow(unused)]

/// 架构名称字符串
pub const ARCH: &str = "loongarch64";

/// QEMU virt 平台 UART 基地址
pub const UART_BASE: usize = 0x1fe001e0;

/// 16 字节对齐所需的掩码
pub const STACK_ALIGN_MASK: usize = 0xF;

/// LoongArch64 地址空间常量
/// 用户空间: 0x0000_0000_0000_0000 - 0x0000_ffff_ffff_ffff
/// 内核空间: 0x9000_0000_0000_0000 - 0xffff_ffff_ffff_ffff
pub const USER_BASE: usize = 0x0000_0000_0000_0000;
pub const USER_TOP: usize = 0x0000_ffff_ffff_ffff;
pub const KERNEL_BASE: usize = 0x9000_0000_0000_0000;

/// 兼容 RISC-V 的地址空间常量
/// 用于与架构无关代码的兼容
pub const SV39_TOP_HALF_TOP: usize = 0xffff_ffff_ffff_ffff;
pub const SV39_TOP_HALF_BASE: usize = KERNEL_BASE;
pub const SV39_BOT_HALF_TOP: usize = USER_TOP;
pub const SV39_BOT_HALF_BASE: usize = USER_BASE;

/// 中断相关常量
pub const IRQ_MIN: usize = usize::MAX / 2;
pub const IRQ_MAX: usize = usize::MAX;
pub const TIMER: usize = usize::MAX / 2 + 1 + 11; // LoongArch 定时器中断
pub const SUPERVISOR_EXTERNAL: usize = usize::MAX / 2 + 1 + 2; // 外部中断

/// CSR 寄存器相关常量
/// CRMD (当前模式信息)
pub const CSR_CRMD_PLV_MASK: usize = 0b11; // 特权级掩码
pub const CSR_CRMD_IE: usize = 1 << 2; // 全局中断使能
pub const CSR_CRMD_DA: usize = 1 << 3; // 直接地址翻译模式
pub const CSR_CRMD_PG: usize = 1 << 4; // 分页使能

/// ECFG (异常配置)
pub const CSR_ECFG_LIE_MASK: usize = 0x1fff; // 局部中断使能掩码

/// ESTAT (异常状态)
pub const CSR_ESTAT_IS_MASK: usize = 0x1fff; // 中断状态掩码

/// sstatus 寄存器兼容常量（用于架构无关代码）
pub const SSTATUS_SIE: usize = CSR_CRMD_IE;
pub const SSTATUS_SPIE: usize = 1 << 3;
pub const SSTATUS_SPP: usize = 1 << 8;
