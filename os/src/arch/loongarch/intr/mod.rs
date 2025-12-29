//! LoongArch64 中断处理模块
#![allow(unused)]

use crate::arch::constant::{CSR_CRMD_IE, CSR_ECFG_LIE_MASK, SSTATUS_SIE};

// CSR 编号
const CRMD_CSR: u32 = 0x0;
const ECFG_CSR: u32 = 0x4;

// 本地中断位
const TIMER_LIE_BIT: usize = 1 << 11; // LIT（Local Interrupt Timer）对应的使能位

#[inline(always)]
unsafe fn set_crmd_ie(enable: bool) -> usize {
    // 读取 CRMD，修改 IE 位后回写，返回旧值
    let old: usize;
    let mut new: usize;
    core::arch::asm!(
        "csrrd {old}, 0x0",
        old = out(reg) old,
        options(nostack, preserves_flags)
    );
    new = if enable {
        old | CSR_CRMD_IE
    } else {
        old & !CSR_CRMD_IE
    };
    core::arch::asm!(
        "csrwr {new}, 0x0",
        new = in(reg) new,
        options(nostack, preserves_flags)
    );
    old
}

#[inline(always)]
unsafe fn read_crmd() -> usize {
    let value: usize;
    unsafe {
        core::arch::asm!(
            "csrrd {value}, 0x0",
            value = out(reg) value,
            options(nostack, preserves_flags)
        );
    }
    value
}

#[inline(always)]
unsafe fn update_ecfg(mask: usize, set: bool) {
    // 只改动给定 mask 覆盖的位
    let mut val: usize;
    core::arch::asm!(
        "csrrd {val}, 0x4",
        val = out(reg) val,
        options(nostack, preserves_flags)
    );
    let bits = mask & CSR_ECFG_LIE_MASK;
    if set {
        val |= bits;
    } else {
        val &= !bits;
    }
    core::arch::asm!(
        "csrwr {val}, 0x4",
        val = in(reg) val,
        options(nostack, preserves_flags)
    );
}

/// 启用定时器中断（仅设置本地定时器使能位，不开启全局 IE）
/// # Safety
/// 直接操作 CSR，调用者需确保时序正确
pub unsafe fn enable_timer_interrupt() {
    unsafe { update_ecfg(TIMER_LIE_BIT, true) };
}

/// 禁用定时器中断（仅清除本地定时器使能位）
/// # Safety
/// 直接操作 CSR，调用者需确保时序正确
pub unsafe fn disable_timer_interrupt() {
    unsafe { update_ecfg(TIMER_LIE_BIT, false) };
}

/// 启用全局中断
/// # Safety
/// 直接操作 CSR 寄存器
pub unsafe fn enable_interrupts() {
    unsafe { set_crmd_ie(true) };
}

/// 禁用全局中断
/// # Safety
/// 直接操作 CSR 寄存器
pub unsafe fn disable_interrupts() {
    unsafe { set_crmd_ie(false) };
}

/// 检查中断是否启用
pub fn is_interrupts_enabled() -> bool {
    unsafe { read_crmd() & CSR_CRMD_IE != 0 }
}

/// 检查中断是否启用（别名）
pub fn are_interrupts_enabled() -> bool {
    is_interrupts_enabled()
}

/// 读取并禁用中断（返回之前的 CRMD 值）
/// # Safety
/// 直接操作 CSR 寄存器
pub unsafe fn read_and_disable_interrupts() -> usize {
    unsafe { set_crmd_ie(false) }
}

/// 读取并启用中断（返回之前的 CRMD 值）
/// # Safety
/// 直接操作 CSR 寄存器
pub unsafe fn read_and_enable_interrupts() -> usize {
    unsafe { set_crmd_ie(true) }
}

/// 恢复中断状态
/// # Safety
/// 直接操作 CSR 寄存器
pub unsafe fn restore_interrupts(flags: usize) {
    if flags & SSTATUS_SIE != 0 {
        unsafe { enable_interrupts() };
    } else {
        unsafe { disable_interrupts() };
    }
}

/// 启用指定 IRQ（LoongArch 目前仅处理本地中断位，其他由中断控制器驱动负责）
pub fn enable_irq(_irq: usize) {
    // 留给外部中断控制器驱动实现
}

/// 禁用指定 IRQ
pub fn disable_irq(_irq: usize) {
    // 留给外部中断控制器驱动实现
}

/// 软中断模块
pub mod softirq {
    /// 初始化软中断
    pub fn init() {}
}
