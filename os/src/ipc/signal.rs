//! 进程间通讯 - 信号
//!
//! 提供软件中断机制，用于发送异步通知给进程
//! 处理信号的捕获、屏蔽和默认行为.
//! # 信号投递流程
//! - **检查：** 内核在返回用户态前，检查私有 `pending` 和共享 `pending`，找到**最高优先级且未被阻塞**的信号 S。
//! - **投递：** 内核将 S 从 `pending` 队列中移除，构建 S 的上下文，并修改 PC/SP 指向信号处理函数。
//! - **返回：** 内核退出，任务在用户态执行信号处理函数。
//! - **循环：** 当信号处理函数执行完毕，通过 `rt_sigreturn` 返回内核后，内核会**再次**进入检查流程。
//!             此时，它可能会发现队列中还有第二个未决信号，然后开始第二次单次投递。

use bitflags::bitflags;

use crate::{
    arch::{
        kernel::cpu,
        trap::{TrapFrame, sigreturn_trampoline_address},
    },
    kernel::{
        SharedTask, TASK_MANAGER, TaskManagerTrait, TaskState, current_cpu, current_task,
        exit_process, exit_task_with_block, sleep_task_with_block, wake_up_with_block, yield_task,
    },
    pr_err,
    uapi::signal::*,
    util::{address::align_down, user_buffer::write_to_user},
};

/// 信号的动作表
/// 每个进程拥有一个独立的信号处理动作表，
/// 用于存储每个信号的处理函数、屏蔽字和标志。
/// 索引 0 未用，信号编号从 1..=_NSIG
/// # SAFE:
/// 该结构体包含裸指针，但其使用受限于内核对任务的锁保护，
/// 因此可以安全地实现 Send 和 Sync。
#[derive(Debug, Clone)]
pub struct SignalHandlerTable {
    /// 动作数组，索引 0 未用，信号编号从 1..=_NSIG
    pub actions: [SignalAction; NSIG + 1],
}

unsafe impl Send for SignalHandlerTable {}
unsafe impl Sync for SignalHandlerTable {}

impl SignalHandlerTable {
    /// 创建一个新的进程信号状态，所有信号动作和屏蔽字为空，待处理信号为空
    pub fn new() -> Self {
        Self {
            actions: core::array::from_fn(|_| SignalAction::default()),
        }
    }

    /// 设置某个信号的动作
    pub fn set_action(&mut self, sig: usize, action: SignalAction) {
        if sig == 0 || sig > NSIG {
            return;
        }
        self.actions[sig] = action;
    }
}

/// 找出第一个可投递的信号（未被屏蔽且挂起的信号中编号最小的）
pub fn first_deliverable_signal(
    pending_flags: SignalFlags,
    blocked_mask: SignalFlags,
) -> Option<SignalFlags> {
    let deliverable_signals = pending_flags.difference(blocked_mask);

    if deliverable_signals.is_empty() {
        return None;
    }

    let signal_bit_index = deliverable_signals.bits().trailing_zeros();
    if signal_bit_index < NSIG as u32 {
        let first_sig = SignalFlags::from_bits(1 << signal_bit_index).unwrap();
        return Some(first_sig);
    }

    None
}

#[inline]
fn handle_one_signal(sig_flag: SignalFlags, action: SignalAction, task: &SharedTask) {
    let sig_num = signal_from_flag(sig_flag).unwrap();

    match unsafe { action.sa_handler() } as isize {
        SIG_DFL => match sig_num {
            NUM_SIGQUIT | NUM_SIGILL | NUM_SIGABRT | NUM_SIGBUS | NUM_SIGFPE | NUM_SIGSEGV
            | NUM_SIGSYS | NUM_SIGXCPU | NUM_SIGXFSZ => sig_dump(sig_num), // 致命错误，调用退出系统调用或内核退出函数

            NUM_SIGHUP | NUM_SIGINT | NUM_SIGPIPE | NUM_SIGALRM | NUM_SIGTERM | NUM_SIGUSR1
            | NUM_SIGUSR2 | NUM_SIGSTKFLT | NUM_SIGPROF | NUM_SIGPWR => sig_terminate(sig_num), // 默认终止
            NUM_SIGKILL => sig_terminate(sig_num), // SIGKILL 总是终止
            NUM_SIGSTOP | NUM_SIGTSTP | NUM_SIGTTIN | NUM_SIGTTOU => sig_stop(sig_num),
            NUM_SIGCONT => sig_continue(sig_num),
            NUM_SIGCHLD | NUM_SIGURG | NUM_SIGWINCH | NUM_SIGIO => sig_ignore(sig_num),
            _ => panic!("Unhandled signal"),
        },
        SIG_IGN => sig_ignore(sig_num),
        handler_addr => {
            // 自定义处理器：构造用户栈上下文并跳转
            install_user_signal_trap_frame(task, sig_num, handler_addr, action);
        }
    }
}

/// 在返回用户态前检查信号并处理
/// # 说明:
/// 该函数会检查当前任务的私有和共享待处理信号集合，
/// 找出第一个可投递的信号并进行处理。
/// 如果没有可投递的信号，则直接返回。
pub fn check_signal() {
    let task = current_task();
    let (sig_flag, action) = {
        let mut t = task.lock();
        let pending_copy = t.pending.clone();
        let shared_pending_copy = t.shared_pending.lock().clone();
        let blocked_copy = t.blocked.clone();
        if let Some(flag) = first_deliverable_signal(pending_copy.signals, blocked_copy) {
            let num = signal_from_flag(flag).unwrap();
            let action = {
                let handlers = t.signal_handlers.lock();
                handlers.actions[num]
            };
            t.pending.signals.remove(flag);
            (flag, action)
        } else if let Some(flag) =
            first_deliverable_signal(shared_pending_copy.signals, blocked_copy)
        {
            let num = signal_from_flag(flag).unwrap();
            let action = {
                let handlers = t.signal_handlers.lock();
                handlers.actions[num]
            };
            t.shared_pending.lock().signals.remove(flag);
            (flag, action)
        } else {
            return;
        }
    };

    handle_one_signal(sig_flag, action, &task);
}

/// 为信号创建 siginfo_t 结构体
/// # 参数:
/// * `flag`: 信号标志
/// TODO: 填充更多字段
pub fn create_siginfo_for_signal(flag: SignalFlags) -> SigInfoT {
    let sig_num = flag.to_signal_number();
    let mut sig_info = SigInfoT::new();
    sig_info.si_signo = sig_num as i32;
    sig_info.si_code = 0;
    sig_info.si_errno = 0;
    sig_info
}

/// 设置信号用户态处理栈帧
/// # 说明:
/// 当信号投递时，内核在信号栈上从高地址到低地址通常依次构建以下结构：
///     1. siginfo_t 结构体。 该结构体包含有关信号的信息（如信号编号、发送者等）。
///     2. ucontext_t 结构体。 该结构体保存了被信号中断时的处理器状态（寄存器等）。
///     3. 返回地址： 指向 C 库中的一个特殊函数（称为 sigreturn 或信号 trampoline），而不是直接返回原程序。
/// 当信号处理函数返回时，它实际上会跳转到栈上的 sigreturn 函数，该函数会调用 rt_sigreturn 系统调用。
/// 内核接收到这个调用后，会从栈上加载 ucontext_t 结构体，恢复所有保存的寄存器状态，从而使程序恢复到被中断时的执行点。
/// # 参数:
/// * `task`: 目标任务
/// * `sig_num`: 信号编号
/// * `entry`: 用户信号处理函数入口地址
/// * `action_mask`: 信号处理函数的屏蔽字
fn install_user_signal_trap_frame(
    task: &SharedTask,
    sig_num: usize,
    entry: isize,
    action: SignalAction,
) {
    let mut t = task.lock();
    let tp = t.trap_frame_ptr.load(core::sync::atomic::Ordering::SeqCst);
    unsafe {
        let tf = &mut *tp;
        let siginfo = create_siginfo_for_signal(SignalFlags::from_signal_num(sig_num).unwrap());
        let sa_flags = SaFlags::from_bits_truncate(action.sa_flags as u32);
        let uc = UContextT::new(
            0,           // TODO: flags未实现
            0 as *mut _, // TODO: link未实现
            t.signal_stack.lock().clone(),
            t.blocked.to_sigset_t(),
            MContextT::from_trap_frame(tf),
        );
        // Linux ABI: build rt_sigframe { siginfo, ucontext } on the user stack.
        let frame_size = core::mem::size_of::<RtSigFrame>();
        let mut sp = align_down(tf.get_sp(), 16);
        sp = align_down(sp - frame_size, 16);

        let sig_info_addr = sp + core::mem::offset_of!(RtSigFrame, info);
        let ucontext_addr = sp + core::mem::offset_of!(RtSigFrame, uc);

        write_to_user(sig_info_addr as *mut SigInfoT, siginfo);
        write_to_user(ucontext_addr as *mut UContextT, uc);

        // 更新 blocked（跳过不可屏蔽信号）
        if sig_num != NUM_SIGKILL && sig_num != NUM_SIGSTOP {
            let self_flag = SignalFlags::from_bits(1 << (sig_num - 1)).unwrap();
            let action_mask = SignalFlags::from_sigset_t(action.sa_mask);
            t.blocked |= action_mask;
            if !sa_flags.contains(SaFlags::NODEFER) {
                t.blocked |= self_flag;
            }
        }

        // Set userspace return address (restorer). Executing a kernel address in U-mode will fault.
        let restorer = if sa_flags.contains(SaFlags::RESTORER) && !action.sa_restorer.is_null() {
            action.sa_restorer as usize
        } else {
            sigreturn_trampoline_address()
        };
        tf.set_ra(restorer);

        // 设置用户处理器入口
        tf.set_sepc(entry as usize);
        tf.set_a0(sig_num);
        tf.set_a1(sig_info_addr);
        tf.set_a2(ucontext_addr);
        tf.set_sp(sp);
    }
}

fn signal_from_flag(flag: SignalFlags) -> Option<usize> {
    let bit_index = flag.bits().trailing_zeros() as usize;

    if bit_index >= NSIG {
        return None;
    }

    Some(bit_index + 1)
}

/* 默认信号处理函数 */
/// 默认行为：进程中止
fn sig_terminate(sig_num: usize) {
    let tasks = TASK_MANAGER.lock().get_process_threads(current_task());
    for task in tasks {
        exit_process(task, (128 + sig_num) as i32);
    }
}

/// 默认行为：终止并 Core Dump
/// TODO: 实现生成 core dump 的功能
fn sig_dump(sig_num: usize) {
    pr_err!("signal {}: generating core (stub)", sig_num);
    sig_terminate(sig_num);
}

/// 默认行为：停止进程
fn sig_stop(sig_num: usize) {
    let pid = current_task().lock().pid;
    let tasks = TASK_MANAGER.lock().get_process_threads(current_task());
    for task in tasks {
        {
            let mut t = task.lock();
            if t.state == TaskState::Zombie {
                continue;
            }
            t.state = TaskState::Stopped;
        }

        // 从运行队列移除（不可被信号唤醒，外部需用 SIGCONT）
        sleep_task_with_block(task, false);
    }
    yield_task();
}

/// 默认行为：继续进程
fn sig_continue(sig_num: usize) {
    let pid = current_task().lock().pid;
    let tasks = TASK_MANAGER.lock().get_process_threads(current_task());
    for task in tasks {
        let mut resume = false;
        {
            let mut t = task.lock();
            if t.state == TaskState::Stopped {
                t.state = TaskState::Running;
                resume = true;
            }
        }
        if resume {
            wake_up_with_block(task);
        }
    }
}

/// 默认行为：忽略信号
fn sig_ignore(sig_num: usize) {}

/// 待处理信号结构体
#[derive(Debug, Clone)]
pub struct SignalPending {
    /// 待处理非实时信号集合
    pub signals: SignalFlags,
    // /// 待处理实时信号队列
    // pub rt_signals: RtSignalQueue,
}

impl SignalPending {
    /// 创建一个空的待处理信号集合
    pub fn empty() -> Self {
        Self {
            signals: SignalFlags::empty(),
            // rt_signals: RtSignalQueue::new(),
        }
    }

    /// 检查是否有可投递的信号
    /// # 参数:
    /// * `blocked`: 当前阻塞的信号集合
    pub fn has_deliverable_signal(&self, blocked: SignalFlags) -> bool {
        !first_deliverable_signal(self.signals, blocked).is_none()
    }

    /// 获取第一个可投递的信号
    /// # 参数:
    /// * `blocked`: 当前阻塞的信号集合
    pub fn first_deliverable_signal(&self, blocked: SignalFlags) -> Option<SignalFlags> {
        first_deliverable_signal(self.signals, blocked)
    }

    /// 获取第一个在目标信号集合中的信号
    /// # 参数:
    /// * `target`: 目标信号集合
    pub fn first_target_signal(&self, target: SignalFlags) -> Option<SignalFlags> {
        let intersect = self.signals & target;
        if intersect.is_empty() {
            return None;
        }

        let signal_bit_index = intersect.bits().trailing_zeros();
        if signal_bit_index < NSIG as u32 {
            let first_sig = SignalFlags::from_bits(1 << signal_bit_index).unwrap();
            return Some(first_sig);
        }

        None
    }
}

/// 获取当前任务的待处理信号集合（私有 + 共享）
pub fn do_sigpending() -> SignalFlags {
    let task = current_task();
    let t = task.lock();
    t.pending.signals | t.shared_pending.lock().signals
}

/// 检查任务是否有可投递的信号
/// # 参数:
/// * `task`: 目标任务
pub fn signal_pending(task: &SharedTask) -> bool {
    let t = task.lock();
    t.pending.has_deliverable_signal(t.blocked)
        || t.shared_pending.lock().has_deliverable_signal(t.blocked)
}

/// Whether a pending signal should interrupt a blocking syscall (i.e. return EINTR).
///
/// We intentionally **do not** treat all pending signals as syscall-interrupting:
/// - Signals with disposition `SIG_IGN` should not interrupt.
/// - Signals with disposition `SIG_DFL` whose default action is "ignore" (e.g. SIGCHLD)
///   should not interrupt. Otherwise daemons like `netserver` can see spurious EINTR and exit.
pub fn signal_interrupts_syscall(task: &SharedTask) -> bool {
    let t = task.lock();
    let pending = t.pending.signals | t.shared_pending.lock().signals;
    let deliverable = pending.difference(t.blocked);
    if deliverable.is_empty() {
        return false;
    }

    let handlers = t.signal_handlers.lock();
    for sig_num in 1..=NSIG {
        // Practical Linux-compat behavior:
        // netserver/netperf often rely on SIGCHLD for child reaping; on Linux this usually does
        // not surface as EINTR to select()/poll() because handlers are installed with SA_RESTART.
        // We don't fully implement SA_RESTART yet, so never use SIGCHLD to interrupt syscalls.
        if sig_num == NUM_SIGCHLD {
            continue;
        }
        let Some(flag) = SignalFlags::from_signal_num(sig_num) else {
            continue;
        };
        if !deliverable.contains(flag) {
            continue;
        }

        let action = handlers.actions[sig_num];
        match unsafe { action.sa_handler() } as isize {
            SIG_IGN => continue,
            SIG_DFL => match sig_num {
                NUM_SIGCHLD | NUM_SIGURG | NUM_SIGWINCH | NUM_SIGIO => continue, // default ignore
                _ => return true,
            },
            _ => return true, // caught
        }
    }

    false
}
