//! RISC-V 架构的中断管理
//!
//! 当前没有接入软中断队列和调度路径，因此这里不保留 `raise_softirq` 这类
//! 会 panic 或静默 no-op 的占位符。后续实现软中断时，应重新引入 softirq
//! 模块，并补齐触发、挂起位管理和处理入口。

use riscv::register::{
    sie,
    sstatus::{self},
};

use crate::arch::constant::{SSTATUS_SIE, SUPERVISOR_EXTERNAL};

/// 启用定时器中断
/// 安全性: 该函数直接操作 CPU 寄存器，启用中断可能会引发竞态条件或不一致状态。
/// 调用者必须确保在适当的上下文中调用此函数，以避免潜在的问题。
pub unsafe fn enable_timer_interrupt() {
    unsafe { sie::set_stimer() };
}

/// 启用软件中断（用于 IPI）
///
/// # Safety
///
/// 该函数直接操作 CPU 寄存器，调用者必须确保在适当的上下文中调用
pub unsafe fn enable_software_interrupt() {
    unsafe { sie::set_ssoft() }
}

/// 启用中断
/// 安全性: 该函数直接操作 CPU 寄存器，启用中断可能会引发竞态条件或不一致状态。
/// 调用者必须确保在适当的上下文中调用此函数，以避免潜在的问题。
pub unsafe fn enable_interrupts() {
    unsafe { sstatus::set_sie() };
}

/// 禁用中断
/// 安全性: 该函数直接操作 CPU 寄存器，禁用中断可能会引发竞态条件或不一致状态。
/// 调用者必须确保在适当的上下文中调用此函数，以避免潜在的问题。
pub unsafe fn disable_interrupts() {
    unsafe { sstatus::clear_sie() };
}

/// 检查中断是否已启用
pub fn are_interrupts_enabled() -> bool {
    sstatus::read().sie()
}

/// 读取并禁用中断，返回之前的中断状态
/// 返回值: 之前的 sstatus 寄存器值
/// 安全性: 该函数直接操作 CPU 寄存器，禁用中断可能会引发竞态条件或不一致状态。
/// 调用者必须确保在适当的上下文中调用此函数，以避免潜在的问题。
#[allow(dead_code)]
pub unsafe fn read_and_disable_interrupts() -> usize {
    let sie_mask: usize = 1 << 1;
    let old: usize;
    unsafe {
        core::arch::asm!(
            "csrrc {old}, sstatus, {mask}",
            old = out(reg) old,
            mask = in(reg) sie_mask,
            options(nomem, nostack)
        );
    }
    old
}

/// 恢复中断状态
/// 参数: flags - 之前的 sstatus 寄存器值
/// 安全性: 该函数直接操作 CPU 寄存器，恢复中断状态可能会引发竞态条件或不一致状态。
/// 调用者必须确保在适当的上下文中调用此函数，以避免潜在的问题。
#[allow(dead_code)]
pub unsafe fn restore_interrupts(flags: usize) {
    let spie: usize = flags & SSTATUS_SIE;
    if spie != 0 {
        unsafe { sstatus::set_sie() };
    }
}

/// 启用指定的中断号
pub fn enable_irq(irq: usize) {
    if irq == SUPERVISOR_EXTERNAL {
        unsafe { sie::set_sext() };
    }
}
