//! RISC-V 架构的陷阱处理程序实现
//!
//! 64 位 RISC-V，usize = 8 字节

use core::sync::atomic::Ordering;

use crate::ipc::check_signal;
use crate::println;
use alloc::task;
use riscv::register::scause::{self, Trap};
use riscv::register::sstatus::SPP;
use riscv::register::{sepc, sscratch, sstatus, stval};

use crate::arch::syscall::dispatch_syscall;
use crate::arch::timer::{TIMER_TICKS, clock_freq, get_time};
use crate::arch::trap::restore;
use crate::kernel::{
    SCHEDULER, TIMER, TIMER_QUEUE, schedule, send_signal_process, wake_up_with_block,
};

/// 陷阱处理程序
/// 从中断处理入口跳转到这里时，
/// 陷阱帧的地址（sp）被隐式地作为参数 a0 传递给了 trap_handler 函数。
/// 在这里，trap_frame 指向了保存的 TrapFrame 结构体。
/// 此外，在通过 stvec 跳转到中断处理入口时，SSIE 被自动清除，
/// 因此在内核陷阱处理程序开始时，中断是被禁用的。
/// 必须从这个函数正常返回后，通过 restore 中的 sret 指令才能正确恢复中断状态。
///
/// # 警告:
/// - 不要在中断处理中调用任何可能引起内存分配或阻塞的函数，
///   因为这可能会导致死锁或不可预测的行为。
/// - 确保在中断处理程序中正确保存和恢复所有必要的寄存器状态，
///   以避免破坏正在运行的进程的状态。
/// - 注意陷阱处理程序的执行时间，
///   避免长时间占用 CPU，影响系统的响应性。
#[unsafe(no_mangle)]
pub extern "C" fn trap_handler(trap_frame: &mut super::TrapFrame) {
    // 保存进入中断时的状态
    let sstatus_old = sstatus::read();
    let sepc_old = sepc::read();
    let scause = scause::read();

    match sstatus_old.spp() {
        SPP::User => user_trap(scause, sepc_old, sstatus_old, trap_frame),
        SPP::Supervisor => kernel_trap(scause, sepc_old, sstatus_old),
    }

    check_signal();
    // Safe:
    // restore 是一个汇编函数，它恢复寄存器并执行 sret。
    //
    // 这里的 unsafe 调用是安全的，前提是：
    // 1. 所有的中断处理逻辑处理已完成。
    // 2. **关键前提：在 match 中调用的 `user_trap` 必须保证如果它修改了 `trap_frame`，
    //    则其中的所有寄存器值（尤其是 sepc 和栈指针）都是有效的、合法的上下文状态。**
    // 3. `restore` 的汇编实现本身是正确的。
    unsafe { restore(trap_frame) };
}

/// 处理来自用户态的陷阱（系统调用、中断、异常）
pub fn user_trap(
    scause: scause::Scause,
    sepc_old: usize,
    sstatus_old: sstatus::Sstatus,
    trap_frame: &mut super::TrapFrame,
) {
    match scause.cause() {
        Trap::Exception(8) => {
            // 设置返回地址为下一个指令
            trap_frame.sepc = sepc_old.wrapping_add(4);
            // 处理系统调用
            dispatch_syscall(trap_frame);
        }
        Trap::Interrupt(5) => {
            // 处理时钟中断
            crate::arch::timer::set_next_trigger();
            check_timer();
        }
        _ => panic!(
            "Unexpected trap in user mode: {:?}, sepc = {:#x}, sstatus = {:#x}",
            scause.cause(),
            sepc_old,
            sstatus_old.bits()
        ),
    }
}

/// 处理来自内核态的陷阱（中断、异常）
pub fn kernel_trap(scause: scause::Scause, sepc_old: usize, sstatus_old: sstatus::Sstatus) {
    match scause.cause() {
        Trap::Interrupt(5) => {
            // 处理时钟中断
            crate::arch::timer::set_next_trigger();
            check_timer();
        }
        // 中断处理时发生异常一般是致命的
        Trap::Exception(e) => {
            // 立即读取 sscratch 和 stval 寄存器的当前值
            let sscratch_val = sscratch::read();
            let stval_val = stval::read();
            let scause_val = scause.bits();

            // 先直接打印到控制台，避免panic格式化时再次触发异常
            println!("\n");
            println!("================ KERNEL PANIC ================");
            println!("Unexpected exception in S-Mode (Kernel)!");
            println!("----------------------------------------------");
            println!("  Exception: {:?} (Raw scause: {:#x})", e, scause_val);
            println!("  Faulting VA (stval): {:#x}", stval_val);
            println!("  Faulting PC (sepc):  {:#x}", sepc_old);
            println!("  sstatus:             {:#x}", sstatus_old.bits());
            println!("  sscratch:            {:#x}", sscratch_val);
            println!("==============================================");
            // sbi::shutdown(true);
            panic!("Kernel exception in S-Mode");
        }
        trap => panic!(
            "Unexpected trap in kernel: {:?}, sepc = {:#x}, sstatus = {:#x}",
            trap,
            sepc_old,
            sstatus_old.bits()
        ),
    }
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
    if SCHEDULER.lock().update_time_slice() {
        schedule();
    }
}

#[allow(dead_code)]
/// TODO: 处理设备中断
pub fn check_device() {}
