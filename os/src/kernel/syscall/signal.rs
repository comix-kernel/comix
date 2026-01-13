//! 信号相关的系统调用实现

use core::{
    ffi::{c_int, c_uint, c_ulong},
    sync::atomic::Ordering,
};

use alloc::{sync::Arc, vec::Vec};

use crate::{
    arch::{timer::clock_freq, trap::restore},
    ipc::{create_siginfo_for_signal, do_sigpending},
    kernel::{
        SharedTask, TASK_MANAGER, TIMER_QUEUE, TaskManagerTrait, current_task,
        sleep_task_with_guard_and_block, yield_task,
    },
    sync::SpinLock,
    uapi::{
        errno::{EAGAIN, EINTR, EINVAL, ENOMEM, ENOSYS, ESRCH},
        signal::{
            MINSIGSTKSZ, NSIG, RtSigFrame, SIG_BLOCK, SIG_SETMASK, SIG_UNBLOCK, SIGSET_SIZE,
            SS_AUTODISARM, SS_DISABLE, SS_ONSTACK, SaFlags, SigInfoT, SignalAction, SignalFlags,
            UContextT,
        },
        time::TimeSpec,
        types::{SigSetT, StackT},
    },
    util::user_buffer::{read_from_user, write_to_user},
};

/// 修改当前任务的信号屏蔽字
/// # 参数：
/// * `how` - 指示如何修改屏蔽字的操作（SIG_BLOCK、SIG_UNBLOCK、SIG_SETMASK）
/// * `set` - 指向用户空间缓冲区的指针，包含要设置的信号集合
/// * `oset` - 指向用户空间缓冲区的指针，用于存放旧的信号集合
/// # 返回值：
/// * 成功时返回 0
/// * 失败时返回负的错误码
pub fn rt_sigprocmask(
    how: c_int,
    set: *const SigSetT,
    oset: *mut SigSetT,
    sigsetsize: c_uint,
) -> c_int {
    if sigsetsize as usize != SIGSET_SIZE {
        return -EINVAL;
    }
    let task = crate::kernel::current_task();
    let mut t = task.lock();

    if !oset.is_null() {
        let old_set = t.blocked.bits() as c_ulong;
        unsafe {
            write_to_user(oset, old_set);
        }
    }

    if !set.is_null() {
        let new_set = unsafe { read_from_user(set) };
        let new_flags = if let Some(flag) = SignalFlags::from_bits(new_set as usize) {
            flag
        } else {
            return -EINVAL;
        };

        match how {
            SIG_BLOCK => {
                t.blocked |= new_flags;
            }
            SIG_UNBLOCK => {
                t.blocked &= !new_flags;
            }
            SIG_SETMASK => {
                t.blocked = new_flags;
            }
            _ => {
                return -EINVAL;
            }
        }
    }

    0
}

/// 获取当前任务的待处理信号集合, 包括私有和共享的信号集合
/// # 参数：
/// * `uset` - 指向用户空间缓冲区的指针，用于存放待处理信号集合
pub fn rt_sigpending(uset: *mut SigSetT, sigsetsize: c_uint) -> c_int {
    if sigsetsize as usize != SIGSET_SIZE {
        return -EINVAL;
    }
    let pending = do_sigpending();
    unsafe {
        write_to_user(uset, pending.bits() as SigSetT);
    }
    0
}

/// 更改指定信号的处理动作
/// # 参数：
/// * `signum` - 信号编号
/// * `act` - 指向新的 SignalAction 结构体的指针（如果不为 NULL）
/// * `oldact` - 指向用于存放旧的 SignalAction 结构体的指针（如果不为 NULL）
/// # 返回值：
/// * 成功时返回 0
/// * 失败时返回负的错误码
pub fn rt_sigaction(signum: c_int, act: *const SignalAction, oldact: *mut SignalAction) -> c_int {
    if signum <= 0 || signum as usize > NSIG {
        return -EINVAL;
    }

    let task = crate::kernel::current_task();
    let t = task.lock();

    if !oldact.is_null() {
        let current_action = t.signal_handlers.lock().actions[signum as usize].clone();
        unsafe {
            write_to_user(oldact, current_action);
        }
    }

    if !act.is_null() {
        let mut new_action = unsafe { read_from_user(act) };
        // Linux ABI compatibility:
        // - libc may pass SA_RESTORER and/or reserved bits.
        // - Rejecting unknown bits causes musl netperf to fail with EINVAL.
        // Keep only known bits and proceed.
        let flag = SaFlags::from_bits_truncate(new_action.sa_flags as u32);
        if !flag.is_supported() {
            return -ENOSYS;
        }
        new_action.sa_flags = flag.bits() as c_ulong;
        t.signal_handlers
            .lock()
            .set_action(signum as usize, new_action);
    }

    0
}

/// 实现实时信号等待
/// # 参数：
/// * `set` - 指向用户空间缓冲区的指针，包含要等待的信号集合
/// * `info` - 指向用户空间缓冲区的指针，用于存放信号信息
/// * `timeout` - 指向用户空间缓冲区的指针，包含超时时间
/// * `sigsetsize` - 信号集合的大小
/// # 返回值：
/// * 成功时返回收到的信号编号
/// * 失败时返回负的错误码
pub fn rt_sigtimedwait(
    set: *const SigSetT,
    info: *mut SigInfoT,
    timeout: *const TimeSpec,
    sigsetsize: c_uint,
) -> c_int {
    if sigsetsize as usize != SIGSET_SIZE {
        return -EINVAL;
    }

    let wait_set_bits = unsafe { read_from_user(set) };
    let wait_set = if let Some(flags) = SignalFlags::from_bits(wait_set_bits as usize) {
        flags
    } else {
        return -EINVAL;
    };

    let timeout_opt = if !timeout.is_null() {
        let ts = unsafe { read_from_user(timeout) };
        Some(ts)
    } else {
        None
    };

    match wait_for_signal(current_task(), wait_set, timeout_opt) {
        Ok((sig_num, sig_info)) => {
            if !info.is_null() {
                unsafe {
                    write_to_user(info, sig_info);
                }
            }
            sig_num as c_int
        }
        Err(err_code) => err_code,
    }
}

/// 实现实时信号挂起
/// 它将三个独立的操作封装成一个不可分割的原子操作:
/// 1. 设置新的信号屏蔽字
/// 2. 挂起当前任务，直到收到信号
/// 3. 恢复旧的信号屏蔽字
/// # 参数：
/// * `unewset` - 指向用户空间缓冲区的指针，包含新的信号集合
/// * `sigsetsize` - 信号集合的大小
/// # 返回值：
/// * 该调用总是被信号中断，返回 -EINTR
/// * 失败时返回负的错误码
pub fn rt_sigsuspend(unewset: *const SigSetT, sigsetsize: c_uint) -> c_int {
    if sigsetsize as usize != SIGSET_SIZE {
        return -EINVAL;
    }
    let new_set_bits = unsafe { read_from_user(unewset) };
    let new_set = if let Some(flags) = SignalFlags::from_bits(new_set_bits as usize) {
        flags
    } else {
        return -EINVAL;
    };
    let task = current_task();
    let old_set;
    {
        let mut t = task.lock();
        old_set = t.blocked;
        t.blocked = new_set;
        sleep_task_with_guard_and_block(&mut t, task.clone(), true);
    }
    yield_task();
    {
        let mut t = task.lock();
        t.blocked = old_set;
    }
    -EINTR
}

/// 撤销之前为了调用信号处理程序而进行的所有操作
/// 利用先前保存在用户空间栈上的信息，
/// 恢复进程的信号掩码，切换栈，并恢复进程的上下文（处理器标志和寄存器，
/// 包括栈指针和指令指针），以便进程从被信号中断的位置恢复执行。
pub fn rt_sigreturn() -> ! {
    let tfp = current_task().lock().trap_frame_ptr.load(Ordering::SeqCst);
    let tf = unsafe { &mut *tfp };
    // Linux ABI: SP points to rt_sigframe { siginfo, ucontext }.
    let frame_addr = tf.get_sp();
    let ucontext_addr = frame_addr + core::mem::offset_of!(RtSigFrame, uc);
    let ucontext: UContextT = unsafe { read_from_user(ucontext_addr as *const UContextT) };

    // Restore blocked mask from saved ucontext.
    {
        let task = current_task();
        let mut t = task.lock();
        t.blocked = SignalFlags::from_bits_truncate(ucontext.uc_sigmask as usize);
    }

    tf.restore_from_mcontext(&ucontext.uc_mcontext);
    unsafe { restore(tf) }
    unreachable!("rt_sigreturn should not return");
}

/// 设置或获取备用信号处理栈的信息
/// # 参数：
/// * `uss` - 指向用户空间缓冲区的指针，包含新的信号栈信息（如果不为 NULL）
/// * `uoss` - 指向用户空间缓冲区的指针，用于存放旧的信号栈信息（如果不为 NULL）
/// # 返回值：
/// * 成功时返回 0
/// * 失败时返回负的错误码
pub fn signal_stack(uss: *const StackT, uoss: *mut StackT) -> c_int {
    let task = current_task();
    let mut t = task.lock();

    if !uoss.is_null() {
        let old_ss = t.signal_stack.lock().clone();
        unsafe {
            write_to_user(uoss, old_ss);
        }
    }

    if !uss.is_null() {
        let new_ss = unsafe { read_from_user(uss) };
        if new_ss.ss_size < MINSIGSTKSZ as u64 {
            return -ENOMEM;
        }
        if new_ss.ss_flags as usize & SS_AUTODISARM & SS_DISABLE & SS_ONSTACK != 0 {
            return -EINVAL;
        }
        t.signal_stack = Arc::new(SpinLock::new(new_ss));
    }
    0
}

/// 向任何进程组或进程发送任何信号。
/// 如果 pid 为正数，则向 pid 指定的进程发送信号 sig。
/// 如果 pid 等于 0，则向调用进程所在进程组中的每个进程发送信号 sig。
/// 如果 pid 等于 -1，则向调用进程有权发送信号的每个进程发送信号 sig，但进程 1（init）除外，
/// 如果 pid 小于 -1，则向进程组 ID 为 -pid 的每个进程发送信号。
/// 如果 sig 为 0，则不发送信号，但仍会执行存在性和权限检查；
/// # 参数：
/// * `pid` - 目标进程或进程组的 ID
/// * `sig` - 要发送的信号编号
/// # 返回值：
/// * 成功时返回 0
/// * 失败时返回负的错误码
pub fn kill(pid: c_int, sig: c_int) -> c_int {
    if sig < 0 || sig as usize >= NSIG {
        return -EINVAL;
    }
    let task_manager = TASK_MANAGER.lock();
    let target_tasks: Vec<SharedTask> = match pid {
        0 => {
            let current_task = current_task();
            let pgid = current_task.lock().pgid;
            task_manager.get_task_cond(|t| {
                t.lock().pgid == pgid && t.lock().pid != 1 && t.lock().is_process()
            })
        }
        pid if pid > 0 => {
            if let Some(task) = task_manager.get_task(pid as u32) {
                if !task.lock().is_process() {
                    return -EINVAL;
                }
                alloc::vec![task]
            } else {
                return -ESRCH;
            }
        }
        -1 => task_manager.get_task_cond(|t| t.lock().pid != 1 && t.lock().is_process()),
        pid if pid < -1 => {
            let pgid = (-pid) as u32;
            task_manager.get_task_cond(|t| {
                t.lock().pgid == pgid && t.lock().pid != 1 && t.lock().is_process()
            })
        }
        _ => alloc::vec![],
    };

    if target_tasks.is_empty() {
        return -ESRCH;
    }

    for task in target_tasks {
        task_manager.send_signal(task, sig as usize);
    }
    0
}

/// 向线程组 tgid 中线程 ID 为 tid 的线程发送信号 sig.
/// 注意: 如果线程终止且其线程 ID 被回收，则向错误的线程发送信号。避免使用此系统调用。
/// # 参数：
/// * `tid` - 目标线程的 ID
/// * `sig` - 要发送的信号编号
/// # 返回值：
/// * 成功时返回 0
/// * 失败时返回负的错误码
pub fn tkill(tid: c_int, sig: c_int) -> c_int {
    if sig < 0 || sig as usize >= NSIG {
        return -EINVAL;
    }
    let task_manager = TASK_MANAGER.lock();
    let task = if let Some(task) = task_manager.get_task(tid as u32) {
        task
    } else {
        return -ESRCH;
    };
    if task.lock().pid != current_task().lock().pid {
        return -EINVAL;
    }
    task_manager.send_signal(task, sig as usize);
    0
}

/// 向线程组 tgid 中线程 ID 为 tid 的线程发送信号 sig。
/// # 参数：
/// * `tgid` - 目标线程组的 ID
/// * `tid` - 目标线程的 ID
/// * `sig` - 要发送的信号编号
/// # 返回值：
/// * 成功时返回 0
/// * 失败时返回负的错误码
pub fn tgkill(tgid: c_int, tid: c_int, sig: c_int) -> c_int {
    if sig < 0 || sig as usize >= NSIG {
        return -EINVAL;
    }
    let task_manager = TASK_MANAGER.lock();
    let task = if let Some(task) = task_manager.get_task(tid as u32) {
        task
    } else {
        return -ESRCH;
    };
    if task.lock().pid != tgid as u32 {
        return -EINVAL;
    }
    task_manager.send_signal(task, sig as usize);
    0
}

/// 在任务中等待指定信号的到来
/// # 参数
/// * `task` - 任务引用
/// * `signal` - 要等待的信号集合
/// * `timeout` - 可选的超时时间
/// # 返回值
/// * 成功时返回收到的信号编号及其信息
/// * 失败时返回负的错误码
fn wait_for_signal(
    task: SharedTask,
    signal: SignalFlags,
    timeout: Option<TimeSpec>,
) -> Result<(usize, SigInfoT), i32> {
    let mut t = task.lock();
    if let Some(timeout) = timeout {
        if timeout.tv_sec < 0 || timeout.tv_nsec < 0 || timeout.tv_nsec >= 1_000_000_000 {
            return Err(-EINVAL);
        }
        if timeout.tv_sec == 0 && timeout.tv_nsec == 0 {
            // 轮询, 不阻塞
            if t.pending.has_deliverable_signal(signal)
                || t.shared_pending.lock().has_deliverable_signal(signal)
            {
                let flag = t
                    .pending
                    .first_deliverable_signal(signal)
                    .or_else(|| t.shared_pending.lock().first_deliverable_signal(signal))
                    .unwrap();
                let sig_num = flag.to_signal_number();
                t.pending.signals.remove(flag);
                return Ok((sig_num, create_siginfo_for_signal(flag)));
            } else {
                Err(-EAGAIN)
            }
        } else {
            // 带超时的阻塞等待
            let start = TimeSpec::now();
            while !t.pending.has_deliverable_signal(signal)
                && !t.shared_pending.lock().has_deliverable_signal(signal)
            {
                let now = TimeSpec::now();
                if now - start > timeout {
                    return Err(-EAGAIN); // 超时返回
                }
                TIMER_QUEUE
                    .lock()
                    .push(timeout.into_freq(clock_freq()), task.clone());
                sleep_task_with_guard_and_block(&mut t, task.clone(), true);
                drop(t);
                yield_task();
                t = task.lock();
            }
            TIMER_QUEUE.lock().remove_task(&task);
            let flag = t
                .pending
                .first_deliverable_signal(signal)
                .or_else(|| t.shared_pending.lock().first_deliverable_signal(signal))
                .unwrap();
            let sig_num = flag.to_signal_number();
            t.pending.signals.remove(flag);
            return Ok((sig_num, create_siginfo_for_signal(flag)));
        }
    } else {
        // 阻塞等待
        while t
            .pending
            .first_target_signal(signal)
            .or_else(|| t.shared_pending.lock().first_target_signal(signal))
            .is_none()
        {
            sleep_task_with_guard_and_block(&mut t, task.clone(), true);
            drop(t);
            yield_task();
            t = task.lock();
        }
        let flag = t
            .pending
            .first_target_signal(signal)
            .or_else(|| t.shared_pending.lock().first_target_signal(signal))
            .unwrap();
        let sig_num = flag.to_signal_number();
        t.pending.signals.remove(flag);
        return Ok((sig_num, create_siginfo_for_signal(flag)));
    }
}
