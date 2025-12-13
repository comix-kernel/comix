//! 定义与等待子进程状态相关的标志。
//!
//! 这些标志用于 `waitpid` 和 `waitid` 系统调用，以指定等待行为。

use core::ffi::c_int;

use bitflags::bitflags;

/// 子进程的状态编码（对应 waitpid/wait4 返回的 wstatus）。
/// 包含了解析和构造状态值的方法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitStatus {
    raw_status: c_int,
}

const __W_CONTINUED: u32 = 0xFFFF; // 0xffff
const __WCOREFLAG: u32 = 0x80; // 0x80 (用于 WCOREDUMP 标记)
const __W_STOP_MAGIC: u32 = 0x7F; // 0x7f (用于 WIFSTOPPED 标记)

impl WaitStatus {
    /// 从原始的 wstatus 整数值创建一个 WaitStatus 实例。
    pub const fn new(raw_status: c_int) -> Self {
        Self { raw_status }
    }

    /// 获取原始状态值。
    pub const fn raw(&self) -> c_int {
        self.raw_status
    }

    /// __W_EXITCODE: 构造一个正常退出或因信号终止的状态值。
    ///
    /// 结构: (Exit Code) << 8 | (Termination Signal)
    /// * ret: 退出码 (0-255)
    /// * sig: 信号编号 (如果为 0 则表示正常退出)
    pub fn exit_code(ret: u8, sig: u8) -> Self {
        let status = ((ret as u32) << 8) | (sig as u32);
        Self::new(status as c_int)
    }

    /// __W_STOPCODE: 构造一个子进程被停止的状态值。
    ///
    /// 结构: (Signal that stopped the child) << 8 | 0x7f
    /// * sig: 导致停止的信号编号
    pub fn stop_code(sig: u8) -> Self {
        let status = ((sig as u32) << 8) | __W_STOP_MAGIC;
        Self::new(status as c_int)
    }

    /// 构造一个子进程从停止状态恢复继续执行的状态值。
    pub fn continued_code() -> Self {
        Self::new(__W_CONTINUED as c_int)
    }

    // WTERMSIG / __WTERMSIG: 获取终止信号编号
    /// 如果 WIFSIGNALED 为真，返回导致子进程终止的信号编号（低 7 位）。
    pub fn termination_signal(&self) -> c_int {
        self.raw_status & 0x7F
    }

    // WIFEXITED / __WIFEXITED: 是否正常退出
    /// 检查状态是否表示正常终止（即终止信号为 0）。
    pub fn is_exited(&self) -> bool {
        self.termination_signal() == 0
    }

    // WEXITSTATUS / __WEXITSTATUS: 获取退出码
    /// 如果 is_exited() 为真，返回子进程的退出状态码（位于第 8-15 位）。
    pub fn exit_status(&self) -> c_int {
        (self.raw_status >> 8) & 0xFF
    }

    // WIFSIGNALED / __WIFSIGNALED: 是否因信号终止
    /// 检查状态是否表示子进程因未捕获的信号而终止。
    pub fn is_signaled(&self) -> bool {
        // C 宏逻辑：((signed char) (((status) & 0x7f) + 1) >> 1) > 0
        // 等价于检查 (status & 0x7f) 是否在 [1, 127] 范围内
        let term_sig = (self.raw_status & 0x7F) as u32;
        term_sig > 0 && term_sig != __W_STOP_MAGIC
    }

    // WIFSTOPPED / __WIFSTOPPED: 是否被停止
    /// 检查状态是否表示子进程被停止信号停止。
    pub fn is_stopped(&self) -> bool {
        (self.raw_status & 0xFF) as u32 == __W_STOP_MAGIC
    }

    // WSTOPSIG / __WSTOPSIG: 获取停止信号编号
    /// 如果 is_stopped() 为真，返回导致子进程停止的信号编号。
    /// 注意：该宏等价于 __WEXITSTATUS(status)，即提取第 8-15 位。
    pub fn stop_signal(&self) -> c_int {
        self.exit_status()
    }

    // WIFCONTINUED / __WIFCONTINUED: 是否已恢复
    /// 检查状态是否表示子进程已从停止状态恢复执行。
    pub fn is_continued(&self) -> bool {
        self.raw_status as u32 == __W_CONTINUED
    }

    // WCOREDUMP / __WCOREDUMP: 是否生成了核心转储
    /// 检查子进程终止时是否生成了核心转储文件。
    pub fn did_core_dump(&self) -> bool {
        // 检查终止信号的最高位是否设置了 0x80 (__WCOREFLAG)
        (self.raw_status as u32) & __WCOREFLAG != 0
    }
}

bitflags! {
    /// 等待子进程状态的选项标志。
    /// 对应于 `waitpid` 和 `waitid` 系统调用中的参数。
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct WaitFlags: usize {
        // --- 适用于 waitpid/wait4 的基本标志 ---

        /// WNOHANG: 非阻塞等待。如果没有子进程状态立即可用，则立即返回 0。
        const NOHANG = 0x1;

        /// WUNTRACED: 报告已停止/终止的子进程状态。
        /// 报告那些因接收到信号（如 SIGSTOP 或 SIGTTIN/SIGTTOU）而停止的子进程。
        const UNTRACED = 0x2;

        // --- 适用于 waitid 的 POSIX/XOPEN 标志 ---

        /// WSTOPPED: 报告已停止的子进程状态（与 WUNTRACED 相同）。
        const STOPPED = 0x2;

        /// WEXITED: 报告已终止（死亡）的子进程状态。
        const EXITED = 0x4;

        /// WCONTINUED: 报告因收到 SIGCONT 信号而恢复执行的子进程。
        const CONTINUED = 0x8;

        /// WNOWAIT: 仅查询状态，不将子进程从 wait set 中移除 (不回收/不释放资源)。
        const NOWAIT = 0x0100_0000;

        // --- Linux/GNU 内部/扩展标志 ---

        /// __WNOTHREAD: 不等待本进程组内其他线程的子进程 (仅限调用线程的子进程)。
        const NOTHREAD = 0x2000_0000;

        /// __WALL: 等待所有子进程，无论其类型（包括通过 clone() 创建的线程）。
        const ALL = 0x4000_0000;

        /// __WCLONE: 仅等待由 clone() 创建的“线程”子进程。
        const CLONE = 0x8000_0000;
    }
}
