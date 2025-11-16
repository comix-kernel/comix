//! 进程间通讯 - 信号
//!
//! 提供软件中断机制，用于发送异步通知给进程
//! 处理信号的捕获、屏蔽和默认行为.

use alloc::task;
use bitflags::bitflags;

use crate::{
    arch::{kernel::cpu, trap::TrapFrame},
    ipc::signal,
    kernel::{SharedTask, current_cpu},
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
        let (deliverable_sig_flag, action) = {
            let mut t = task.lock();

            let Some(sig_flag) = first_deliverable_signal(t.pending, t.blocked) else {
                break;
            };

            let sig_num = signal_from_flag(sig_flag).unwrap();
            let action = t.signal_handlers.lock().actions[sig_num];

            t.pending.remove(sig_flag);

            (sig_flag, action)
        };

        handle_one_signal(deliverable_sig_flag, action, &task);

        // 注意：如果 handle_one_signal 导致进程退出 (sig_terminate/sig_dump)，
        // 那么循环将不会继续，而是会被退出逻辑接管。
    }
}

/// 设置信号用户态处理跳板
fn install_user_signal_trampoline(
    task: &SharedTask,
    sig_num: usize,
    entry: usize,
    action_mask: SignalFlags, // 接收 action.mask 作为参数
) {
    let tp = current_cpu()
        .lock()
        .current_task
        .clone()
        .unwrap()
        .lock()
        .trap_frame_ptr
        .load(core::sync::atomic::Ordering::SeqCst);

    let original_blocked = {
        let mut t = task.lock();
        let original = t.blocked;

        // 更新 blocked mask（原子性）
        // 新的屏蔽集 = 原有屏蔽集 | 动作屏蔽集 | 自身信号
        let sig_flag = SignalFlags::from_bits(1 << (sig_num - 1)).unwrap();
        t.blocked |= action_mask | sig_flag;

        original
    };

    unsafe {
        // 构造 Signal Frame (在用户栈上)
        let user_sp = (*tp).x2_sp;
        let frame_size = size_of::<TrapFrame>();
        let mask_size = size_of::<SignalFlags>();
        let frame_addr = user_sp
            .checked_sub(frame_size + mask_size)
            .expect("User stack exhausted for Signal Frame");
        let mask_addr = frame_addr + frame_size;
        let frame_ptr = frame_addr as *mut TrapFrame;
        (*frame_ptr).clone_from(&*tp);
        let mask_ptr = mask_addr as *mut SignalFlags;
        *mask_ptr = original_blocked;

        // 修改内核 Trap Frame (实现跳转)
        (*tp).sepc = entry;
        (*tp).x10_a0 = sig_num;
        // (*tp).x1_ra = TRAMPOLINE_SIGRETURN_ADDR;
        (*tp).x2_sp = frame_addr;
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
    unimplemented!()
}

/// 默认行为：终止并 Core Dump
fn sig_dump(sig_num: usize) {
    unimplemented!()
}

/// 默认行为：停止进程
fn sig_stop(sig_num: usize) {
    unimplemented!()
}

/// 默认行为：继续进程
fn sig_continue(sig_num: usize) {
    unimplemented!()
}

/// 默认行为：忽略信号
fn sig_ignore(sig_num: usize) {}
