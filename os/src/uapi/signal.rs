//! 信号处理行为标志和相关常量定义
//! 
//! 这些常量用于信号处理函数的行为控制，
//! 以及信号屏蔽操作等。

use bitflags::bitflags;

// --- 信号处理行为标志 (SA_FLAGS) ---
// 用于 struct sigaction 的 sa_flags 字段，控制信号处理函数的行为。
bitflags! {
    /// 信号处理行为标志，用于 sigaction 系统调用。
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct SaFlags: u32 {
        /// 阻止 SIGCHLD 在子进程停止时生成。
        const NOCLDSTOP = 0x0000_0001;

        /// 在 SIGCHLD 上设置，以禁止僵尸进程 (子进程退出时不产生僵尸进程)。
        const NOCLDWAIT = 0x0000_0002;

        /// 使用 siginfo_t 结构体传递信号详细信息 (开启实时信号行为，使用三参数处理器)。
        const SIGINFO = 0x0000_0004;

        /// 表示将使用已注册的备用信号栈 (sigaltstack)。
        const ONSTACK = 0x0800_0000;

        /// 标志，使得被中断的慢速系统调用在信号处理函数返回后自动重新启动。
        const RESTART = 0x1000_0000;

        /// 防止当前信号在处理函数执行期间被自动阻塞 (即，不自动添加到屏蔽字)。
        const NODEFER = 0x4000_0000;

        /// 清除信号处理函数：信号在投递后，其处理函数被重置为 SIG_DFL。
        const RESETHAND = 0x8000_0000;

        /// 不受支持的标志位。用于用户空间检测内核对标志位的支持情况。
        const UNSUPPORTED = 0x0000_0400;

        /// 暴露架构定义的标签位（tag bits）到 siginfo.si_addr 中。
        const EXPOSE_TAGBITS = 0x0000_0800;
        
        // --- 历史名称别名 ---
        /// 历史上的 SA_NOMASK 名称，等同于 SA_NODEFER。
        const NOMASK = Self::NODEFER.bits();

        /// 历史上的 SA_ONESHOT 名称，等同于 SA_RESETHAND.
        const ONESHOT = Self::RESETHAND.bits();
    }
}


// --- 信号屏蔽操作常量 (sigprocmask / rt_sigprocmask) ---
// 用于 sigprocmask 函数的 how 参数，指示如何修改信号屏蔽字。

/// 将信号集添加到当前的屏蔽字中（阻塞信号）。
pub const SIG_BLOCK: i32 = 0;

/// 从当前的屏蔽字中移除信号集（解除阻塞）。
pub const SIG_UNBLOCK: i32 = 1;

/// 将当前的屏蔽字设置为给定的信号集。
pub const SIG_SETMASK: i32 = 2;


// --- 信号处理函数特殊值 ---

// 这里的处理动作值（SIG_DFL, SIG_IGN, SIG_ERR）在 C 语言中是特殊的指针值，
// 在 Rust 中，我们使用 isize 来模拟这些与指针相关的常量。

/// 默认信号处理：由内核执行默认动作 (终止、停止、忽略等)。
pub const SIG_DFL: isize = 0;

/// 忽略信号：内核将忽略该信号。
pub const SIG_IGN: isize = 1;

/// 错误返回值：表示信号系统调用的错误。
pub const SIG_ERR: isize = -1;
