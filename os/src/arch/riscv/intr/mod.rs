//! RISC-V 架构的中断管理
#![allow(unused)]
use riscv::register::{
    sie,
    sstatus::{self, Sstatus},
};

/// 启用定时器中断
pub unsafe fn enable_timer_interrupt() {
    unsafe { sie::set_stimer() };
}

/// 禁用定时器中断
pub unsafe fn disable_timer_interrupt() {
    unsafe { sie::clear_stimer() };
}

/// 启用中断
pub unsafe fn enable_interrupts() {
    unsafe { sstatus::set_sie() };
}

/// 禁用中断
pub unsafe fn disable_interrupts() {
    unsafe { sstatus::clear_sie() };
}

/// 检查中断是否已启用
pub fn are_interrupts_enabled() -> bool {
    sstatus::read().sie()
}

/// 读取并禁用中断，返回之前的中断状态
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

/// 恢复中断状态
pub unsafe fn restore_interrupts(flags: usize) {
    // XXX: 与 read_and_disable_interrupts 配合使用时，中断禁用期间其它并发可能会改变寄存器状态，此时要恢复它们吗？
    unsafe { sstatus::write(Sstatus::from_bits(flags)) };
}
