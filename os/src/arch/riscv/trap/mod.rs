pub mod kerneltrap;
pub mod usertrap;

use core::arch::global_asm;
use riscv::register::{
    mtvec::TrapMode,
    stvec::{self, Stvec},
};

use crate::arch::trap::usertrap::TrapFrame;

global_asm!(include_str!("kernelvec.S"));
global_asm!(include_str!("trampoline.S"));
global_asm!(include_str!("restore.S"));

unsafe extern "C" {
    unsafe fn kernelvec();
}

pub fn init() {
    set_kernel_trap_entry();
}

fn set_kernel_trap_entry() {
    unsafe {
        stvec::write(Stvec::new(kernelvec as usize, TrapMode::Direct));
    }
}

unsafe extern "C" {
    pub unsafe fn __restore(trap_frame: &TrapFrame);
}
