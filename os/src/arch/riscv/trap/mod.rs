pub mod kerneltrap;

use core::arch::global_asm;
use riscv::register::{
    mtvec::TrapMode,
    stvec::{self, Stvec},
};

global_asm!(include_str!("kernelvec.S"));
global_asm!(include_str!("trampoline.S"));

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
