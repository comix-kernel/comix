//! 进程间通讯 - 信号
//!
//! 提供软件中断机制，用于发送异步通知给进程
//! 处理信号的捕获、屏蔽和默认行为.

use bitflags::bitflags;

use crate::{arch::kernel::cpu, kernel::current_cpu};

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
                handler: 0,
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
