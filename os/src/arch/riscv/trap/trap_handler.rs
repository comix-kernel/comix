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
    // 检查进入trap_handler时的中断状态
    let sie_on_entry = sstatus::read().sie();
    if sie_on_entry {
        crate::earlyprintln!("[WARN] trap_handler: SIE=1 on entry! Interrupts should be disabled!");
    }

    // 保存进入中断时的状态
    let sstatus_old = sstatus::read();
    let sepc_old = sepc::read();
    let scause = scause::read();

    // 验证trap_frame的sepc与CSR一致
    if trap_frame.sepc != sepc_old {
        crate::earlyprintln!("[WARN] trap_frame.sepc={:#x} != sepc_old={:#x}", trap_frame.sepc, sepc_old);
    }

    match sstatus_old.spp() {
        SPP::User => {
            let current_task = crate::kernel::current_task();
            let canary_ok = {
                let t = current_task.lock();
                t.check_kstack_canary()
            };
            if !canary_ok {
                let tid = current_task.lock().tid;
                crate::earlyprintln!("[FATAL] kstack canary corrupted: tid={}", tid);
                panic!("kstack overflow detected");
            }
            user_trap(scause, sepc_old, sstatus_old, trap_frame);

            // 检查sepc是否被破坏
            if trap_frame.sepc == 0 {
                crate::earlyprintln!("[ERROR] trap_frame.sepc corrupted to 0 after user_trap!");
                crate::earlyprintln!("  scause={:?}, sepc_old={:#x}", scause.cause(), sepc_old);
            }

            check_signal();

            // 最终检查
            if trap_frame.sepc == 0 {
                let tid = crate::kernel::current_cpu().lock().current_task.as_ref().map(|t| t.lock().tid).unwrap_or(0);
                let tp = crate::kernel::current_cpu().lock().current_task.as_ref()
                    .map(|t| t.lock().trap_frame_ptr.load(core::sync::atomic::Ordering::SeqCst)).unwrap_or(core::ptr::null_mut());
                crate::earlyprintln!("[ERROR] trap_frame.sepc corrupted to 0 after check_signal!");
                crate::earlyprintln!("  tid={}, trap_frame={:p}, trap_frame_ptr={:p}", tid, trap_frame, tp);
                panic!("sepc corrupted to 0 before restore");
            }
        }
        SPP::Supervisor => kernel_trap(scause, sepc_old, sstatus_old),
    }
    // Safe:
    // restore 是一个汇编函数，它恢复寄存器并执行 sret。
    //
    // 这里的 unsafe 调用是安全的，前提是：
    // 1. 所有的中断处理逻辑处理已完成。
    // 2. **关键前提：在 match 中调用的 `user_trap` 必须保证如果它修改了 `trap_frame`，
    //    则其中的所有寄存器值（尤其是 sepc 和栈指针）都是有效的、合法的上下文状态。**
    // 3. `restore` 的汇编实现本身是正确的。

    // 最终验证
    if sstatus_old.spp() == SPP::User && trap_frame.sepc == 0 {
        crate::earlyprintln!("[FATAL] About to restore with sepc=0!");
        crate::earlyprintln!("  trap_frame addr: {:p}", trap_frame);
        crate::earlyprintln!("  sepc_old was: {:#x}", sepc_old);
        panic!("Refusing to restore with sepc=0");
    }

    // 在restore前再次验证trap_frame内容
    if sstatus_old.spp() == SPP::User {
        let sepc_value = unsafe { core::ptr::read_volatile(&trap_frame.sepc) };
        let a0_value = unsafe { core::ptr::read_volatile(&trap_frame.x10_a0) };
        if sepc_value == 0 {
            crate::earlyprintln!("[FATAL] trap_frame.sepc is 0 right before restore!");
            crate::earlyprintln!("  trap_frame addr: {:p}", trap_frame);
            crate::earlyprintln!("  a0={:#x}", a0_value);
            panic!("trap_frame corrupted before restore");
        }
    }

    unsafe { restore(trap_frame) };
}

/// 处理来自用户态的陷阱（系统调用、中断、异常）
pub fn user_trap(
    scause: scause::Scause,
    sepc_old: usize,
    sstatus_old: sstatus::Sstatus,
    trap_frame: &mut super::TrapFrame,
) {
    // 检查sepc是否异常
    if sepc_old == 0 || sepc_old < 0x1000 {
        let tid = crate::kernel::current_cpu().lock().current_task.as_ref().map(|t| t.lock().tid).unwrap_or(0);
        let task = crate::kernel::current_cpu().lock().current_task.clone();

        crate::earlyprintln!("[ERROR] user_trap: tid={}, sepc_old={:#x} is invalid!", tid, sepc_old);
        crate::earlyprintln!("  scause={:?}", scause.cause());
        crate::earlyprintln!("  ra={:#x}, sp={:#x}", trap_frame.x1_ra, trap_frame.x2_sp);
        crate::earlyprintln!("  a0={:#x}, a7={:#x}", trap_frame.x10_a0, trap_frame.x17_a7);

        // Check stack
        if let Some(t) = task {
            let task_lock = t.lock();
            let kstack_size = task_lock.kstack_size();
            let kstack_bottom = task_lock.kstack_bottom();
            let kstack_top = task_lock.kstack_base;
            let kstack_guard_top = task_lock.kstack_guard_top();
            let current_sp = trap_frame.x2_sp;

            crate::earlyprintln!("  [Stack] kstack: {:#x}..{:#x} (size={})",
                kstack_bottom, kstack_top, kstack_size);
            crate::earlyprintln!("  [Stack] current sp={:#x}", current_sp);

            if current_sp < 0x1000 || current_sp >= 0xffffffc000000000 {
                crate::earlyprintln!("  [Stack] ERROR: sp in kernel space!");
            } else if current_sp < kstack_guard_top {
                crate::earlyprintln!("  [Stack] ERROR: sp in guard page!");
            } else if current_sp < kstack_bottom || current_sp >= kstack_top {
                crate::earlyprintln!("  [Stack] WARNING: sp outside kstack range");
            }
        }
    }

    // crate::pr_debug!("[user_trap] scause: {:?}", scause.cause());
    match scause.cause() {
        Trap::Exception(8) => {
            // 设置返回地址为下一个指令
            trap_frame.sepc = sepc_old.wrapping_add(4);
            // 处理系统调用
            dispatch_syscall(trap_frame);

            // 验证sepc没有被破坏
            if trap_frame.sepc == 0 || trap_frame.sepc < 0x1000 {
                crate::earlyprintln!("[FATAL] syscall corrupted sepc to {:#x}!", trap_frame.sepc);
                crate::earlyprintln!("  sepc_old was: {:#x}", sepc_old);
                panic!("sepc corrupted after syscall");
            }

            // Log syscall return for debugging
            if trap_frame.x17_a7 == 63 {  // read syscall
                let task = crate::kernel::current_cpu().lock().current_task.clone();
                if let Some(t) = task {
                    let task_lock = t.lock();
                    let kstack_size = task_lock.kstack_size();
                    let kstack_bottom = task_lock.kstack_bottom();
                    let kstack_top = task_lock.kstack_base;
                    let tf_addr = trap_frame as *const _ as usize;

                    crate::pr_debug!("[syscall] read() returning, sepc={:#x}, ra={:#x}, a0={}",
                        trap_frame.sepc, trap_frame.x1_ra, trap_frame.x10_a0 as isize);
                    crate::pr_debug!("  kstack: {:#x}..{:#x}, trap_frame at {:#x}",
                        kstack_bottom, kstack_top, tf_addr);

                    if tf_addr < kstack_bottom || tf_addr >= kstack_top {
                        crate::pr_debug!("  [WARN] trap_frame outside kstack!");
                    }
                }
            }
        }
        Trap::Interrupt(5) => {
            // 处理时钟中断 - 中断不改变PC
            trap_frame.sepc = sepc_old;
            crate::arch::timer::set_next_trigger();

            // 检查调用check_timer前的中断状态
            let sie_before = sstatus::read().sie();
            crate::pr_debug!("[Timer] Before check_timer: SIE={}", sie_before);

            check_timer();

            // 检查调用check_timer后的中断状态
            let sie_after = sstatus::read().sie();
            if sie_after != sie_before {
                crate::earlyprintln!("[ERROR] check_timer changed SIE: {} -> {}", sie_before, sie_after);
            }
        }
        Trap::Interrupt(9) => {
            // 处理外部中断（设备中断） - 中断不改变PC
            trap_frame.sepc = sepc_old;
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

            panic!(
                "Unexpected trap in user mode: {:?}, sepc = {:#x}, stval = {:#x}, sstatus = {:#x}",
                scause.cause(),
                sepc_old,
                stval_val,
                sstatus_old.bits()
            );
        }
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
        Trap::Interrupt(9) => {
            // 处理外部中断（设备中断）
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
        let sie_before_schedule = riscv::register::sstatus::read().sie();
        crate::pr_debug!("[Timer] Before schedule: SIE={}", sie_before_schedule);

        schedule();

        let sie_after_schedule = riscv::register::sstatus::read().sie();
        if sie_after_schedule != sie_before_schedule {
            crate::earlyprintln!("[ERROR] schedule changed SIE: {} -> {}", sie_before_schedule, sie_after_schedule);
        }
    }
}

/// 处理设备中断
pub fn check_device() {
    crate::device::IRQ_MANAGER.read().try_handle_interrupt(None);
}
