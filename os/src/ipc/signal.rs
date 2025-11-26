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
    arch::{kernel::cpu, trap::TrapFrame},
    kernel::{
        SharedTask, TASK_MANAGER, TaskManagerTrait, TaskState, current_cpu, current_task,
        exit_process, exit_task_with_block, sleep_task_with_block, wake_up_with_block, yield_task,
    },
    pr_err,
};

/* 信号定义 */
pub const _NSIG: usize = 31;

pub const _SIGHUP: usize = 1;
pub const _SIGINT: usize = 2;
pub const _SIGQUIT: usize = 3;
pub const _SIGILL: usize = 4;
pub const _SIGTRAP: usize = 5;
pub const _SIGABRT: usize = 6;
pub const _SIGBUS: usize = 7;
pub const _SIGFPE: usize = 8;
pub const _SIGKILL: usize = 9;
pub const _SIGUSR1: usize = 10;
pub const _SIGSEGV: usize = 11;
pub const _SIGUSR2: usize = 12;
pub const _SIGPIPE: usize = 13;
pub const _SIGALRM: usize = 14;
pub const _SIGTERM: usize = 15;
pub const _SIGSTKFLT: usize = 16;
pub const _SIGCHLD: usize = 17;
pub const _SIGCONT: usize = 18;
pub const _SIGSTOP: usize = 19;
pub const _SIGTSTP: usize = 20;
pub const _SIGTTIN: usize = 21;
pub const _SIGTTOU: usize = 22;
pub const _SIGURG: usize = 23;
pub const _SIGXCPU: usize = 24;
pub const _SIGXFSZ: usize = 25;
pub const _SIGVTALRM: usize = 26;
pub const _SIGPROF: usize = 27;
pub const _SIGWINCH: usize = 28;
pub const _SIGIO: usize = 29;
pub const _SIGPWR: usize = 30;
pub const _SIGSYS: usize = 31;

bitflags! {
    #[derive(Clone, Debug, Copy)]
    pub struct SignalFlags: usize {
        const SIGHUP = 1 << (_SIGHUP - 1);
        const SIGINT = 1 << (_SIGINT - 1);
        const SIGQUIT = 1 << (_SIGQUIT - 1);
        const SIGILL = 1 << (_SIGILL - 1);
        const SIGTRAP = 1 << (_SIGTRAP - 1);
        const SIGABRT = 1 << (_SIGABRT - 1);
        const SIGBUS = 1 << (_SIGBUS - 1);
        const SIGFPE = 1 << (_SIGFPE - 1);
        const SIGKILL = 1 << (_SIGKILL - 1);
        const SIGUSR1 = 1 << (_SIGUSR1 - 1);
        const SIGSEGV = 1 << (_SIGSEGV - 1);
        const SIGUSR2 = 1 << (_SIGUSR2 - 1);
        const SIGPIPE = 1 << (_SIGPIPE - 1);
        const SIGALRM = 1 << (_SIGALRM - 1);
        const SIGTERM = 1 << (_SIGTERM - 1);
        const SIGSTKFLT = 1 << (_SIGSTKFLT - 1);
        const SIGCHLD = 1 << (_SIGCHLD - 1);
        const SIGCONT = 1 << (_SIGCONT - 1);
        const SIGSTOP = 1 << (_SIGSTOP - 1);
        const SIGTSTP = 1 << (_SIGTSTP - 1);
        const SIGTTIN = 1 << (_SIGTTIN - 1);
        const SIGTTOU = 1 << (_SIGTTOU - 1);
        const SIGURG = 1 << (_SIGURG - 1);
        const SIGXCPU = 1 << (_SIGXCPU - 1);
        const SIGXFSZ = 1 << (_SIGXFSZ - 1);
        const SIGVTALRM = 1 << (_SIGVTALRM - 1);
        const SIGPROF = 1 << (_SIGPROF - 1);
        const SIGWINCH = 1 << (_SIGWINCH - 1);
        const SIGIO = 1 << (_SIGIO - 1);
        const SIGPWR = 1 << (_SIGPWR - 1);
        const SIGSYS = 1 << (_SIGSYS - 1);
    }
}

impl SignalFlags {
    pub fn from_signal_num(sig_num: usize) -> Option<Self> {
        if sig_num == 0 || sig_num > _NSIG {
            return None;
        }
        Some(SignalFlags::from_bits(1 << (sig_num - 1)).unwrap())
    }
}

/// 默认信号处理动作
pub const SIG_DEF: usize = 0;
/// 忽略信号
pub const SIG_IGN: usize = 1;

/// 信号处理动作
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct SignalAction {
    /// 信号处理函数指针
    pub handler: usize,
    /// 信号屏蔽字
    pub mask: SignalFlags,
}

/// 信号的动作表
#[derive(Debug, Clone)]
pub struct SignalHandlerTable {
    /// 动作数组，索引 0 未用，信号编号从 1..=_NSIG
    pub actions: [SignalAction; _NSIG + 1],
}

impl SignalHandlerTable {
    /// 创建一个新的进程信号状态，所有信号动作和屏蔽字为空，待处理信号为空
    pub fn new() -> Self {
        Self {
            actions: core::array::from_fn(|_| SignalAction {
                handler: SIG_DEF,
                mask: SignalFlags::empty(),
            }),
        }
    }

    /// 设置某个信号的动作
    pub fn set_action(&mut self, sig: usize, action: SignalAction) {
        if sig == 0 || sig > _NSIG {
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
    if signal_bit_index < _NSIG as u32 {
        let first_sig = SignalFlags::from_bits(1 << signal_bit_index).unwrap();
        return Some(first_sig);
    }

    None
}

#[inline]
fn handle_one_signal(sig_flag: SignalFlags, action: SignalAction, task: &SharedTask) {
    let sig_num = signal_from_flag(sig_flag).unwrap();

    match action.handler {
        SIG_DEF => match sig_num {
            _SIGQUIT | _SIGILL | _SIGABRT | _SIGBUS | _SIGFPE | _SIGSEGV | _SIGSYS | _SIGXCPU
            | _SIGXFSZ => sig_dump(sig_num), // 致命错误，调用退出系统调用或内核退出函数

            _SIGHUP | _SIGINT | _SIGPIPE | _SIGALRM | _SIGTERM | _SIGUSR1 | _SIGUSR2
            | _SIGSTKFLT | _SIGPROF | _SIGPWR => sig_terminate(sig_num), // 默认终止
            _SIGKILL => sig_terminate(sig_num), // SIGKILL 总是终止
            _SIGSTOP | _SIGTSTP | _SIGTTIN | _SIGTTOU => sig_stop(sig_num),
            _SIGCONT => sig_continue(sig_num),
            _SIGCHLD | _SIGURG | _SIGWINCH | _SIGIO => sig_ignore(sig_num),
            _ => panic!("Unhandled signal"),
        },
        SIG_IGN => sig_ignore(sig_num),
        handler_addr => {
            // 自定义处理器：构造用户栈上下文并跳转
            // **将 action.mask 传递给安装跳板函数**
            install_user_signal_trap_frame(task, sig_num, handler_addr, action.mask);
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

/// 设置信号用户态处理栈帧
/// # 说明:
/// 当信号投递时，内核在信号栈上从高地址到低地址通常依次构建以下结构：
///     1. 返回地址： 指向 C 库中的一个特殊函数（称为 sigreturn 或信号 trampoline），而不是直接返回原程序。
///     2. siginfo_t 结构体。 该结构体包含有关信号的信息（如信号编号、发送者等）。
///     3. ucontext_t 结构体。 该结构体保存了被信号中断时的处理器状态（寄存器等）。
///     4. 信号处理函数的参数： 准备好传递给用户注册的信号处理函数的参数
///        （通常是信号编号、siginfo_t* 指针和 ucontext_t* 指针）。
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
    entry: usize,
    action_mask: SignalFlags,
) {
    use core::mem::size_of;
    let mut t = task.lock();
    let tp = t.trap_frame_ptr.load(core::sync::atomic::Ordering::SeqCst);
    unsafe {
        let tf = &mut *tp;
        let user_sp = tf.x2_sp;
        let frame_size = size_of::<TrapFrame>();
        let mask_size = size_of::<SignalFlags>();
        // 16 字节对齐
        let total = (frame_size + mask_size + 15) & !15;
        let frame_addr = user_sp.checked_sub(total).expect("signal frame overflow");
        let frame_ptr = frame_addr as *mut TrapFrame;
        frame_ptr.write(*tf); // 保存旧上下文
        let mask_ptr = (frame_addr + frame_size) as *mut SignalFlags;
        mask_ptr.write(t.blocked.clone());

        // 更新 blocked（跳过不可屏蔽信号）
        if sig_num != _SIGKILL && sig_num != _SIGSTOP {
            let self_flag = SignalFlags::from_bits(1 << (sig_num - 1)).unwrap();
            t.blocked |= action_mask | self_flag;
        }

        // 设置用户处理器入口
        tf.sepc = entry;
        tf.x10_a0 = sig_num;
        tf.x2_sp = frame_addr;
    }
}

fn signal_from_flag(flag: SignalFlags) -> Option<usize> {
    let bit_index = flag.bits().trailing_zeros() as usize;

    if bit_index >= _NSIG {
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
    let pid = current_cpu()
        .lock()
        .current_task
        .clone()
        .unwrap()
        .lock()
        .pid;
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
    let pid = current_cpu()
        .lock()
        .current_task
        .clone()
        .unwrap()
        .lock()
        .pid;
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
}

/// 用户态信号处理上下文结构体
/// 该结构体包含恢复进程执行所需的所有状态。它是 POSIX 标准定义的上下文结构体之一。
#[repr(C)]
struct UContext {
    /// 旧的 TrapFrame，上下文切换前的寄存器状态
    pub old_trap_frame: TrapFrame,
    /// 信号掩码
    /// 信号处理函数执行时生效的新的阻塞信号集。
    /// 这是由 sigaction 注册信号处理函数时指定的 sa_mask 字段确定的。
    pub us_sigmask: SignalFlags,
    /// 备用栈信息
    /// 记录当前激活的栈信息（如果正在使用备用信号栈）。
    pub uc_stack: SignalStack,
    /// 上下文链接
    /// 指向下一个 ucontext_t 结构体的指针。
    /// 用于处理嵌套的信号处理程序或在 makecontext() 等非本地跳转时使用。
    pub uc_link: usize,
}

/// 信号信息结构体
/// 该结构体用于向信号处理函数传递有关信号的信息。
#[repr(C)]
struct SignalInfo {
    /// 信号编号
    pub si_signo: usize,
    // /// 信号的来源代码
    // pub si_code: isize,
    // /// 错误号
    // pub csi_errno: isize,
    /// 发送者进程 ID
    pub si_pid: usize,
    // /// 发送者用户 ID
    // pub si_uid: usize,
}

/// 信号栈信息结构体
/// 该结构体用于描述备用信号栈的信息。
#[repr(C)]
struct SignalStack {
    /// 栈顶地址
    pub ss_sp: usize,
    /// 栈大小
    pub ss_size: usize,
    /// 栈状态标志
    pub ss_flags: usize,
}

/// 获取当前任务的待处理信号集合（私有 + 共享）
pub fn do_sigpending() -> SignalFlags {
    let task = current_task();
    let t = task.lock();
    t.pending.signals | t.shared_pending.lock().signals
}
