mod kerneltrap;

use core::arch::global_asm;
use riscv::register::{
    mtvec::TrapMode, sie, stvec::{self, Stvec},
};

global_asm!(include_str!("kernelvec.S"));

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

pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

pub fn enable_interrupts() {
    unsafe {
        use riscv::register::sstatus; 
        sstatus::set_sie();
    }
}