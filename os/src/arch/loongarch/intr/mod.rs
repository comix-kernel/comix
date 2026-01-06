//! LoongArch64 中断处理模块

use loongArch64::register::{crmd, ecfg};
use loongArch64::register::ecfg::LineBasedInterrupt;

use crate::arch::constant::{CSR_CRMD_IE, TIMER};

/// 启用中断
/// # Safety
/// 直接操作 CSR 寄存器
pub unsafe fn enable_interrupts() {
    crmd::set_ie(true);
}

/// 禁用中断
/// # Safety
/// 直接操作 CSR 寄存器
pub unsafe fn disable_interrupts() {
    crmd::set_ie(false);
}

/// 检查中断是否启用
pub fn is_interrupts_enabled() -> bool {
    crmd::read().ie()
}

/// 检查中断是否启用（别名）
pub fn are_interrupts_enabled() -> bool {
    is_interrupts_enabled()
}

/// 读取并禁用中断（返回之前的状态）
pub fn read_and_disable_interrupts() -> usize {
    let old = crmd::read().raw();
    crmd::set_ie(false);
    old
}

/// 读取并启用中断
pub fn read_and_enable_interrupts() -> usize {
    let old = crmd::read().raw();
    crmd::set_ie(true);
    old
}

/// 恢复中断状态
pub fn restore_interrupts(flags: usize) {
    if (flags & CSR_CRMD_IE) != 0 {
        crmd::set_ie(true);
    } else {
        crmd::set_ie(false);
    }
}

/// 启用指定 IRQ
pub fn enable_irq(_irq: usize) {
    match _irq {
        TIMER => unsafe { enable_timer_interrupt() },
        _ => {
            // TODO: 非定时器中断由平台中断控制器负责启用
        }
    }
}

/// 禁用指定 IRQ
pub fn disable_irq(_irq: usize) {
    match _irq {
        TIMER => unsafe { disable_timer_interrupt() },
        _ => {
            // TODO: 非定时器中断由平台中断控制器负责关闭
        }
    }
}

/// 启用定时器中断
/// # Safety
/// 直接操作 CSR 寄存器
pub unsafe fn enable_timer_interrupt() {
    let mut lie = ecfg::read().lie();
    lie |= LineBasedInterrupt::TIMER;
    ecfg::set_lie(lie);
}

/// 禁用定时器中断
/// # Safety
/// 直接操作 CSR 寄存器
pub unsafe fn disable_timer_interrupt() {
    let mut lie = ecfg::read().lie();
    lie.remove(LineBasedInterrupt::TIMER);
    ecfg::set_lie(lie);
}

/// 软中断模块
pub mod softirq {
    /// 初始化软中断
    pub fn init() {}
}
