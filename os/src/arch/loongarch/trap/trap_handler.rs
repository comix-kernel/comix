//! LoongArch64 trap handler implementation

use loongArch64::register::{badv, crmd, era, estat, prmd};

use crate::arch::syscall::dispatch_syscall;
use crate::arch::trap::restore;
use crate::ipc::check_signal;

#[unsafe(no_mangle)]
pub extern "C" fn trap_handler(trap_frame: &mut super::TrapFrame) {
    let estat_val = estat::read();
    let era_val = era::read().pc();
    let prmd_val = prmd::read();
    let crmd_val = crmd::read();

    trap_frame.era = era_val;
    trap_frame.estat = estat_val.raw();
    trap_frame.prmd = prmd_val.raw();
    trap_frame.crmd = crmd_val.raw();

    match prmd_val.pplv() {
        3 => user_trap(estat_val, era_val, trap_frame),
        _ => kernel_trap(estat_val, era_val),
    }

    check_signal();

    unsafe { restore(trap_frame) };
}

fn user_trap(estat_val: estat::Estat, era_val: usize, trap_frame: &mut super::TrapFrame) {
    match estat_val.cause() {
        estat::Trap::Exception(estat::Exception::Syscall) => {
            trap_frame.era = era_val.wrapping_add(4);
            dispatch_syscall(trap_frame);
        }
        estat::Trap::Interrupt(_irq) => {
            // TODO: 处理中断（定时器/设备）
        }
        _ => {
            let badv_val = badv::read().vaddr();
            panic!(
                "Unexpected user trap: {:?}, era={:#x}, badv={:#x}",
                estat_val.cause(),
                era_val,
                badv_val
            );
        }
    }
}

fn kernel_trap(estat_val: estat::Estat, era_val: usize) {
    let badv_val = badv::read().vaddr();
    panic!(
        "Unexpected kernel trap: {:?}, era={:#x}, badv={:#x}",
        estat_val.cause(),
        era_val,
        badv_val
    );
}
