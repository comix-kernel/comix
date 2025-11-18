//! 进程间通讯 - 信号
//!
//! 提供软件中断机制，用于发送异步通知给进程
//! 处理信号的捕获、屏蔽和默认行为.

use alloc::task;
use bitflags::bitflags;

use crate::{
    arch::{kernel::cpu, trap::TrapFrame},
    ipc::signal,
    kernel::{
        SharedTask, TASK_MANAGER, TaskManagerTrait, TaskState, current_cpu, do_exit,
        exit_task_with_block, sleep_task_with_block, wake_up_with_block,
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

/// 默认信号处理动作
pub const SIG_DEF: usize = 0;
/// 忽略信号
pub const SIG_IGN: usize = 1;

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
/// 信号处理动作
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
            actions: [SignalAction {
                handler: SIG_DEF,
                mask: SignalFlags::empty(),
            }; _NSIG + 1],
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
            install_user_signal_trampoline(task, sig_num, handler_addr, action.mask);
        }
    }
}

/// 在返回用户态前检查信号并处理
pub fn check_signal() {
    let task = {
        let cpu = current_cpu().lock();
        cpu.current_task.as_ref().unwrap().clone()
    };
    loop {
        let (sig_flag, action) = {
            let mut t = task.lock();
            let Some(flag) = first_deliverable_signal(t.pending, t.blocked) else {
                break;
            };
            let num = signal_from_flag(flag).unwrap();
            let action = {
                let handlers = t.signal_handlers.lock();
                handlers.actions[num]
            };
            t.pending.remove(flag);
            (flag, action)
        };

        handle_one_signal(sig_flag, action, &task);

        // 若任务已退出或停止且为终止类信号，停止继续处理
        {
            let t = task.lock();
            if t.state == TaskState::Zombie {
                break;
            }
        }
    }
}

/// 设置信号用户态处理跳板
fn install_user_signal_trampoline(
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
        mask_ptr.write(t.blocked);

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
    let pid = current_cpu()
        .lock()
        .current_task
        .clone()
        .unwrap()
        .lock()
        .pid;
    let tasks = TASK_MANAGER.lock().get_process_threads(pid);
    for task in tasks {
        do_exit(task, (128 + sig_num) as i32);
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
    let tasks = TASK_MANAGER.lock().get_process_threads(pid);
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
    let tasks = TASK_MANAGER.lock().get_process_threads(pid);
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
