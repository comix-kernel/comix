//! RISC-V 架构的陷阱处理程序实现
//!
//! 64 位 RISC-V，usize = 8 字节

use core::sync::atomic::Ordering;

use crate::earlyprintln;
use crate::ipc::check_signal;
use riscv::register::scause::{self, Trap};
use riscv::register::sstatus::SPP;
use riscv::register::{sepc, sscratch, sstatus, stval};

use crate::arch::syscall::dispatch_syscall;
use crate::arch::timer::{TIMER_TICKS, clock_freq, get_time};
use crate::arch::trap::restore;
use crate::arch::constant::SUPERVISOR_EXTERNAL;
use crate::device::IRQ_MANAGER;
use crate::kernel::{TIMER, TIMER_QUEUE, schedule, send_signal_process, wake_up_with_block};

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
        SPP::User => {
            user_trap(scause, sepc_old, sstatus_old, trap_frame);
            // 仅在返回用户态时检查信号
            check_signal();
        }
        SPP::Supervisor => kernel_trap(scause, sepc_old, sstatus_old),
    }
    // 恢复“当前任务”的陷阱帧。
    // 注意：在陷阱处理中可能发生了调度（例如用户态定时器中断），
    // 这时需要恢复到新任务的 TrapFrame，而不是入口参数 trap_frame。
    let tf_ptr = crate::kernel::try_current_task()
        .map(|t| {
            t.lock()
                .trap_frame_ptr
                .load(core::sync::atomic::Ordering::SeqCst) as usize
        })
        .unwrap_or(trap_frame as *mut _ as usize);
    // SAFETY: 指针来源于当前任务保存的 trap_frame_ptr 或回退到入口参数。
    unsafe { restore(&*(tf_ptr as *const super::TrapFrame)) };
}

/// 处理来自用户态的陷阱（系统调用、中断、异常）
pub fn user_trap(
    scause: scause::Scause,
    sepc_old: usize,
    sstatus_old: sstatus::Sstatus,
    trap_frame: &mut super::TrapFrame,
) {
    crate::pr_debug!("[user_trap] scause: {:?}", scause.cause());
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
        Trap::Interrupt(1) => {
            // 软件中断（IPI）：仅当有待运行任务时才调度，避免空转
            crate::arch::ipi::handle_ipi();
            let need_sched = {
                let sched = crate::kernel::current_scheduler().lock();
                !sched.is_empty()
            };
            if need_sched {
                schedule();
            }
        }
        Trap::Interrupt(9) => {
            // 外部中断（设备）
            check_device();
        }
        _ => {
            // 立即读取相关寄存器的当前值
            let stval_val = stval::read();
            let scause_val = scause.bits();
            let sscratch_val = sscratch::read();

            // 打印详细的异常信息
            crate::earlyprintln!("\n");
            crate::earlyprintln!("===============================================");
            crate::earlyprintln!("   UNEXPECTED TRAP IN USER MODE (U-Mode)");
            crate::earlyprintln!("===============================================");
            crate::earlyprintln!("");
            crate::earlyprintln!("[!] Exception Type:");
            crate::earlyprintln!("   Trap: {:?}", scause.cause());
            crate::earlyprintln!("   Raw scause: {:#x}", scause_val);
            crate::earlyprintln!("");
            crate::earlyprintln!("[!] Exception Location:");
            crate::earlyprintln!("   sepc (fault PC):  {:#x}", sepc_old);
            crate::earlyprintln!("   stval (fault VA): {:#x}", stval_val);
            crate::earlyprintln!("");
            crate::earlyprintln!("[!] Register State:");
            crate::earlyprintln!("   sstatus: {:#x}", sstatus_old.bits());
            crate::earlyprintln!("   sscratch: {:#x}", sscratch_val);
            crate::earlyprintln!("");
            crate::earlyprintln!("[!] TrapFrame Details:");
            crate::earlyprintln!("   Stack Pointers:");
            crate::earlyprintln!("     x2_sp (user stack): {:#x} <===", trap_frame.x2_sp);
            crate::earlyprintln!("     kernel_sp:          {:#x}", trap_frame.kernel_sp);
            crate::earlyprintln!("");
            crate::earlyprintln!("   General Registers:");
            crate::earlyprintln!(
                "     x1_ra:  {:#x}  x3_gp:  {:#x}  x4_tp:  {:#x}",
                trap_frame.x1_ra,
                trap_frame.x3_gp,
                trap_frame.x4_tp
            );
            crate::earlyprintln!(
                "     x5_t0:  {:#x}  x6_t1:  {:#x}  x7_t2:  {:#x}",
                trap_frame.x5_t0,
                trap_frame.x6_t1,
                trap_frame.x7_t2
            );
            crate::earlyprintln!("");
            crate::earlyprintln!("   Argument Registers:");
            crate::earlyprintln!(
                "     x10_a0: {:#x}  x11_a1: {:#x}  x12_a2: {:#x}",
                trap_frame.x10_a0,
                trap_frame.x11_a1,
                trap_frame.x12_a2
            );
            crate::earlyprintln!(
                "     x13_a3: {:#x}  x14_a4: {:#x}  x15_a5: {:#x}",
                trap_frame.x13_a3,
                trap_frame.x14_a4,
                trap_frame.x15_a5
            );
            crate::earlyprintln!(
                "     x16_a6: {:#x}  x17_a7: {:#x}",
                trap_frame.x16_a6,
                trap_frame.x17_a7
            );
            crate::earlyprintln!("");
            crate::earlyprintln!("   Saved Registers:");
            crate::earlyprintln!(
                "     x8_s0:  {:#x}  x9_s1:  {:#x}",
                trap_frame.x8_s0,
                trap_frame.x9_s1
            );
            crate::earlyprintln!(
                "     x18_s2: {:#x}  x19_s3: {:#x}",
                trap_frame.x18_s2,
                trap_frame.x19_s3
            );
            crate::earlyprintln!("");

            // 解释常见的异常类型
            crate::earlyprintln!("[!] Exception Explanation:");
            match scause.cause() {
                Trap::Exception(0) => {
                    crate::earlyprintln!("   Instruction Address Misaligned");
                    crate::earlyprintln!("   -> PC address {:#x} is not 2-byte aligned", sepc_old);
                }
                Trap::Exception(1) => {
                    crate::earlyprintln!("   Instruction Access Fault");
                    crate::earlyprintln!("   -> Cannot fetch instruction from {:#x}", sepc_old);
                }
                Trap::Exception(2) => {
                    crate::earlyprintln!("   Illegal Instruction");
                    crate::earlyprintln!("   -> Illegal instruction at {:#x}", sepc_old);
                }
                Trap::Exception(4) => {
                    crate::earlyprintln!("   Load Address Misaligned");
                    crate::earlyprintln!(
                        "   -> Tried to load from misaligned address {:#x}",
                        stval_val
                    );
                }
                Trap::Exception(5) => {
                    crate::earlyprintln!("   Load Access Fault");
                    crate::earlyprintln!("   -> Cannot read from address {:#x}", stval_val);
                }
                Trap::Exception(6) => {
                    crate::earlyprintln!("   Store/AMO Address Misaligned");
                    crate::earlyprintln!(
                        "   -> Tried to store to misaligned address {:#x}",
                        stval_val
                    );
                }
                Trap::Exception(7) => {
                    crate::earlyprintln!("   Store/AMO Access Fault");
                    crate::earlyprintln!("   -> Cannot write to address {:#x}", stval_val);
                }
                Trap::Exception(12) => {
                    crate::earlyprintln!("   Instruction Page Fault");
                    crate::earlyprintln!(
                        "   -> Page table entry invalid or no permission at {:#x}",
                        sepc_old
                    );
                }
                Trap::Exception(13) => {
                    crate::earlyprintln!("   Load Page Fault");
                    crate::earlyprintln!(
                        "   -> Page table entry invalid or no read permission at {:#x}",
                        stval_val
                    );
                }
                Trap::Exception(15) => {
                    crate::earlyprintln!("   Store Page Fault");
                    crate::earlyprintln!(
                        "   -> Page table entry invalid or no write permission at {:#x}",
                        stval_val
                    );
                }
                _ => {
                    crate::earlyprintln!("   Unknown exception type");
                }
            }
            crate::earlyprintln!("");
            crate::earlyprintln!("===============================================");
            // 不要因为用户态异常让内核 panic；仿照 Linux 行为，终止当前任务即可。
            // TODO: 进一步完善为向进程投递对应信号（SIGILL/SIGSEGV/...），并支持 core dump 等。
            let sig = match scause.cause() {
                Trap::Exception(2) => crate::uapi::signal::NUM_SIGILL, // Illegal Instruction
                Trap::Exception(12) | Trap::Exception(13) | Trap::Exception(15) => {
                    crate::uapi::signal::NUM_SIGSEGV
                }
                _ => crate::uapi::signal::NUM_SIGILL,
            };
            crate::kernel::terminate_task(128 + sig);
        }
    }
}

/// 处理来自内核态的陷阱（中断、异常）
pub fn kernel_trap(scause: scause::Scause, sepc_old: usize, sstatus_old: sstatus::Sstatus) {
    match scause.cause() {
        Trap::Interrupt(5) => {
            // 时钟中断（内核态）
            // 1) 设置下一次触发
            // 2) 驱动内核定时器与唤醒队列（与用户态路径一致），避免 CPU 停在 idle 时错过唤醒
            // 3) 若有可运行任务，或当前正处于 idle 任务，则立即调度
            crate::arch::timer::set_next_trigger();

            // 驱动 TIMER/TIMER_QUEUE，唤醒超时任务
            check_timer();

            // 是否需要在内核态进行一次调度：
            // - 运行队列非空；或
            // - 当前任务就是 idle（典型 WFI 返回场景）
            let need_sched = {
                let sched = crate::kernel::current_scheduler().lock();
                !sched.is_empty()
            };
            let is_idle = {
                let cpu = crate::kernel::current_cpu();
                if let (Some(cur), Some(idle)) = (&cpu.current_task, &cpu.idle_task) {
                    alloc::sync::Arc::ptr_eq(cur, idle)
                } else {
                    false
                }
            };
            if need_sched || is_idle {
                schedule();
            }
        }
        Trap::Interrupt(1) => {
            // 软件中断（IPI）：仅当运行队列非空时触发调度
            crate::arch::ipi::handle_ipi();
            let need_sched = {
                let sched = crate::kernel::current_scheduler().lock();
                !sched.is_empty()
            };
            if need_sched {
                schedule();
            }
        }
        Trap::Interrupt(9) => {
            // 外部中断（设备）
            check_device();
        }
        // 中断处理时发生异常一般是致命的
        Trap::Exception(e) => {
            // 立即读取 sscratch 和 stval 寄存器的当前值
            let sscratch_val = sscratch::read();
            let stval_val = stval::read();
            let scause_val = scause.bits();

            // 先直接打印到控制台，避免panic格式化时再次触发异常
            earlyprintln!("\n");
            earlyprintln!("================ KERNEL PANIC ================");
            earlyprintln!("Unexpected exception in S-Mode (Kernel)!");
            earlyprintln!("----------------------------------------------");
            earlyprintln!("  Exception: {:?} (Raw scause: {:#x})", e, scause_val);
            earlyprintln!("  Faulting VA (stval): {:#x}", stval_val);
            earlyprintln!("  Faulting PC (sepc):  {:#x}", sepc_old);
            earlyprintln!("  sstatus:             {:#x}", sstatus_old.bits());
            earlyprintln!("  sscratch:            {:#x}", sscratch_val);
            earlyprintln!("==============================================");
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

    // 推进网络栈，避免在仅有 loopback/null-net 且任务阻塞在 select/poll 时网络停滞。
    // 在时钟中断里推进一次网络栈，保证即便缺少真实网卡中断也能推进 TCP 状态机/重传等。
    crate::net::socket::poll_network_interfaces();
    crate::kernel::syscall::io::wake_poll_waiters();

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
    // 仅在时间片用尽且运行队列非空时才触发调度，避免空转日志刷屏
    let do_sched = {
        let mut sched = crate::kernel::current_scheduler().lock();
        sched.update_time_slice() && !sched.is_empty()
    };
    if do_sched {
        schedule();
    }
}

#[allow(dead_code)]
/// 处理设备中断
pub fn check_device() {
    IRQ_MANAGER
        .read()
        .try_handle_interrupt(Some(SUPERVISOR_EXTERNAL));
}
