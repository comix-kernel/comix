//! RISC-V 架构的中断管理
#![allow(unused)]
mod softirq;

use riscv::register::{
    sie,
    sstatus::{self, Sstatus},
};
pub use softirq::*;

use crate::{
    arch::{
        constant::{IRQ_MIN, SSTATUS_SIE},
        mm::paddr_to_vaddr,
    },
    println,
};

/// 启用定时器中断
/// 安全性: 该函数直接操作 CPU 寄存器，启用中断可能会引发竞态条件或不一致状态。
/// 调用者必须确保在适当的上下文中调用此函数，以避免潜在的问题。
pub unsafe fn enable_timer_interrupt() {
    unsafe { sie::set_stimer() };
}

/// 禁用定时器中断
/// 安全性: 该函数直接操作 CPU 寄存器，禁用中断可能会引发竞态条件或不一致状态。
/// 调用者必须确保在适当的上下文中调用此函数，以避免潜在的问题。
pub unsafe fn disable_timer_interrupt() {
    unsafe { sie::clear_stimer() };
}

/// 启用软件中断（用于 IPI）
///
/// # Safety
///
/// 该函数直接操作 CPU 寄存器，调用者必须确保在适当的上下文中调用
pub unsafe fn enable_software_interrupt() {
    sie::set_ssoft()
}

/// 禁用软件中断
///
/// # Safety
///
/// 该函数直接操作 CPU 寄存器，调用者必须确保在适当的上下文中调用
pub unsafe fn disable_software_interrupt() {
    sie::clear_ssoft()
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
pub unsafe fn read_and_disable_interrupts() -> usize {
    // SIE 在 sstatus 的位 1
    let sie_mask: usize = 1 << 1;
    let old: usize;
    // 原子地清除 sstatus 中的 SIE 位，并返回旧的 sstatus 值
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

///读取并启用中断
pub unsafe fn read_and_enable_interrupts() -> usize {
    // SIE 在 sstatus 的位 1
    let sie_mask: usize = 1 << 1;
    let old: usize;
    // 原子地设置 sstatus 中的 SIE 位，并返回旧的 sstatus 值
    unsafe {
        core::arch::asm!(
        "csrrs {old}, sstatus, {mask}",
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
pub unsafe fn restore_interrupts(flags: usize) {
    let spie: usize = flags & SSTATUS_SIE;
    if spie != 0 {
        unsafe { sstatus::set_sie() };
    }
}

/// 启用指定的中断号
pub fn enable_irq(irq: usize) {
    // Handled in PLIC driver
}
