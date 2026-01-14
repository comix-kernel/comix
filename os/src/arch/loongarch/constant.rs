//! LoongArch64 架构常量定义
#![allow(unused)]

/// 架构名称字符串
pub const ARCH: &str = "loongarch64";

/// QEMU virt 平台 UART 基地址
pub const UART_BASE: usize = 0x1fe001e0;

/// 16 字节对齐所需的掩码
pub const STACK_ALIGN_MASK: usize = 0xF;

/// LoongArch64 地址空间常量
/// 用户空间: 0x0000_0000_0000_0000 - 0x0000_003f_ffff_ffff (39-bit)
/// 内核空间: 0x9000_0000_0000_0000 - 0xffff_ffff_ffff_ffff
pub const USER_BASE: usize = 0x0000_0000_0000_0000;
pub const USER_TOP: usize = 0x0000_003f_ffff_ffff;
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
pub const CSR_CRMD_DATF_MASK: usize = 0b11 << 5; // 取指访问类型
pub const CSR_CRMD_DATM_MASK: usize = 0b11 << 7; // 读写访问类型
pub const CSR_CRMD_DAT_CC: usize = 0b01; // Coherent Cached

/// PRMD (异常前模式信息)
pub const PRMD_PPLV_MASK: usize = 0b11;
pub const PRMD_PIE: usize = 1 << 2;
pub const PRMD_PPLV_USER: usize = 0b11;

/// ECFG (异常配置)
pub const CSR_ECFG_LIE_MASK: usize = 0x1fff; // 局部中断使能掩码

/// EENTRY (异常入口地址)
pub const CSR_EENTRY: u32 = 0xc;
/// TLBRENT (TLB refill 入口地址)
pub const CSR_TLBRENT: u32 = 0x88;

/// BADV (错误地址寄存器)
pub const CSR_BADV: u32 = 0x7;
/// BADI (错误指令寄存器)
pub const CSR_BADI: u32 = 0x8;

/// ESTAT (异常状态)
pub const CSR_ESTAT_IS_MASK: usize = 0x1fff; // 中断状态掩码

/// sstatus 寄存器兼容常量（用于架构无关代码）
pub const SSTATUS_SIE: usize = CSR_CRMD_IE;
pub const SSTATUS_SPIE: usize = 1 << 3;
pub const SSTATUS_SPP: usize = 1 << 8;
