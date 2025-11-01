mod trap_handler;

use core::arch::global_asm;
use riscv::register::{
    mtvec::TrapMode,
    stvec::{self, Stvec},
};

use crate::arch::trap;
pub use crate::arch::trap::trap_handler::TrapFrame;

global_asm!(include_str!("trap_entry.S"));
global_asm!(include_str!("boot_trap_entry.S"));

/// 初始化引导时的陷阱处理程序
pub fn init_boot_trap() {
    set_boot_trap_entry();
}

/// 初始化陷阱处理程序
pub fn init() {
    set_trap_entry();
}

/// 恢复到陷阱前的上下文
pub fn restore(trap_frame: &TrapFrame) {
    unsafe { __restore(trap_frame) };
}

fn set_trap_entry() {
    unsafe {
        stvec::write(Stvec::new(trap_entry as usize, TrapMode::Direct));
    }
}

fn set_boot_trap_entry() {
    unsafe {
        stvec::write(Stvec::new(boot_trap_entry as usize, TrapMode::Direct));
    }
}

unsafe extern "C" {
    unsafe fn boot_trap_entry();
    unsafe fn trap_entry();
    unsafe fn __restore(trap_frame: &TrapFrame);
}
