//! RISC-V 架构的陷阱处理模块
//!
//! 包含陷阱处理程序的实现
mod sum_guard;
mod trap_frame;
mod trap_handler;

use core::arch::global_asm;
use riscv::register::{
    mtvec::TrapMode,
    stvec::{self, Stvec},
};

pub use sum_guard::SumGuard;
pub use trap_frame::TrapFrame;

global_asm!(include_str!("trap_entry.S"));
global_asm!(include_str!("boot_trap_entry.S"));
global_asm!(include_str!("sigreturn.S"));

/// 初始化引导时的陷阱处理程序
pub fn init_boot_trap() {
    set_boot_trap_entry();
}

/// 初始化陷阱处理程序
pub fn init() {
    set_trap_entry();
}

/// 恢复到陷阱前的上下文
/// # Safety
/// 该函数涉及直接操作处理器状态，必须确保传入的 TrapFrame 是有效且正确的。
pub unsafe fn restore(trap_frame: &TrapFrame) {
    unsafe { __restore(trap_frame) };
}

/// 获取信号返回的 trampoline 地址
pub fn sigreturn_trampoline_address() -> usize {
    __sigreturn_trampoline as usize
}

fn set_trap_entry() {
    // Safe: 仅在内核初始化阶段调用，确保唯一性
    unsafe {
        stvec::write(Stvec::new(trap_entry as usize, TrapMode::Direct));
    }
}

fn set_boot_trap_entry() {
    // Safe: 仅在内核初始化阶段调用，确保唯一性
    unsafe {
        stvec::write(Stvec::new(boot_trap_entry as usize, TrapMode::Direct));
    }
}

unsafe extern "C" {
    unsafe fn boot_trap_entry();
    unsafe fn trap_entry();
    unsafe fn __restore(trap_frame: &TrapFrame);
    unsafe fn __sigreturn_trampoline();
}
