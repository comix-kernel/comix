//! LoongArch64 trap handler implementation

use core::sync::atomic::Ordering;

use loongArch64::register::ticlr;
use loongArch64::register::{badv, crmd, era, estat, prmd};

use crate::arch::syscall::dispatch_syscall;
use crate::arch::timer::{TIMER_TICKS, clock_freq, get_time};
use crate::arch::trap::restore;
use crate::ipc::check_signal;
use crate::kernel::{
    SCHEDULER, TIMER, TIMER_QUEUE, schedule, send_signal_process, wake_up_with_block,
};

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

    if crate::kernel::current_cpu()
        .lock()
        .current_task
        .is_some()
    {
        check_signal();
    }

    unsafe { restore(trap_frame) };
}

fn user_trap(estat_val: estat::Estat, era_val: usize, trap_frame: &mut super::TrapFrame) {
    match estat_val.cause() {
        estat::Trap::Exception(estat::Exception::Syscall) => {
            trap_frame.era = era_val.wrapping_add(4);
            dispatch_syscall(trap_frame);
        }
        estat::Trap::Interrupt(irq) => match irq {
            estat::Interrupt::Timer => handle_timer_interrupt(),
            _ => check_device(),
        },
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
    match estat_val.cause() {
        estat::Trap::Interrupt(irq) => match irq {
            estat::Interrupt::Timer => handle_timer_interrupt(),
            _ => check_device(),
        },
        _ => {
            let badv_val = badv::read().vaddr();
            panic!(
                "Unexpected kernel trap: {:?}, era={:#x}, badv={:#x}",
                estat_val.cause(),
                era_val,
                badv_val
            );
        }
    }
}

fn handle_timer_interrupt() {
    crate::arch::timer::set_next_trigger();
    ticlr::clear_timer_interrupt();
    check_timer();
}

/// 处理时钟中断
pub fn check_timer() {
    let _ticks = TIMER_TICKS.fetch_add(1, Ordering::Relaxed);
    while let Some(task) = TIMER_QUEUE.lock().pop_due_task(get_time()) {
        wake_up_with_block(task);
    }
    while let Some(entry) = TIMER.lock().pop_due_entry(get_time()) {
        send_signal_process(&entry.task, entry.sig);
        if !entry.it_interval.is_zero() {
            let next_trigger = get_time() + entry.it_interval.into_freq(clock_freq());
            TIMER.lock().push(next_trigger, entry);
        }
    }
    if crate::kernel::current_cpu()
        .lock()
        .current_task
        .is_some()
        && SCHEDULER.lock().update_time_slice()
    {
        schedule();
    }
}

#[allow(dead_code)]
pub fn check_device() {}
