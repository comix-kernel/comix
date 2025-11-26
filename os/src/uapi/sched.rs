//! 任务与调度相关的常量和结构体定义
//!
//! 源于 Linux 内核头文件 <linux/sched.h> 中定义的常量和结构体

use bitflags::bitflags;
use core::ffi::{c_int, c_ulonglong};

bitflags! {
    /// 用于 clone() 系统调用的标志位，控制子任务的资源共享和行为。
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct CloneFlags: usize {
        // 低 8 位用于指定子进程退出时发送的信号编号，通常是 SIGCHLD。
        const CSIGNAL = 0x000000FF;

        /// 设置时，父子任务共享相同的虚拟内存空间。
        /// 线程 (Thread) 实现的关键。
        const VM = 0x00000100;
        /// 设置时，父子任务共享文件系统信息 (如当前工作目录、根目录)。
        const FS = 0x00000200;
        /// 设置时，父子任务共享相同的文件描述符表。
        const FILES = 0x00000400;
        /// 设置时，父子任务共享信号处理器和阻塞信号集。
        const SIGHAND = 0x00000800;

        /// 设置时，在父进程中返回一个指向子进程的 PID 文件描述符 (pidfd)。
        const PIDFD = 0x00001000;
        /// 设置时，允许 ptrace 调试器继续跟踪子进程。
        const PTRACE = 0x00002000;
        /// 设置时，父进程会等待子进程释放内存映射 (mm_release) 后再继续执行。
        /// 模仿 vfork 的行为。
        const VFORK = 0x00004000;
        /// 设置时，新任务的父进程与调用者的父进程相同 (绕过调用者作为父进程)。
        const PARENT = 0x00008000;
        /// 设置时，子任务与父任务属于同一个线程组 (Thread Group)，即创建线程。
        const THREAD = 0x00010000;

        // --- 命名空间标志 (用于容器) ---
        /// 创建新的 Mount 命名空间。
        const NEWNS = 0x00020000;
        /// 共享 System V SEM_UNDO 语义 (用于信号量)。
        const SYSVSEM = 0x00040000;
        /// 为子任务创建新的线程局部存储 (TLS)。
        const SETTLS = 0x00080000;
        /// 在父进程内存中设置子任务的 TID。
        const PARENT_SETTID = 0x00100000;
        /// 在子任务退出时，清除子任务内存中的 TID。
        const CHILD_CLEARTID = 0x00200000;
        /// 已弃用，会被忽略。
        const DETACHED = 0x00400000;
        /// 设置时，跟踪进程不能强制设置 CLONE_PTRACE。
        const UNTRACED = 0x00800000;
        /// 在子任务内存中设置子任务的 TID。
        const CHILD_SETTID = 0x01000000;
        /// 创建新的 Cgroup 命名空间。
        const NEWCGROUP = 0x02000000;
        /// 创建新的 UTS 命名空间 (主机名和域名)。
        const NEWUTS = 0x04000000;
        /// 创建新的 IPC 命名空间。
        const NEWIPC = 0x08000000;
        /// 创建新的 User 命名空间。
        const NEWUSER = 0x10000000;
        /// 创建新的 PID 命名空间。
        const NEWPID = 0x20000000;
        /// 创建新的 Network 命名空间。
        const NEWNET = 0x40000000;
        /// 克隆 IO 上下文。
        const IO = 0x80000000;

        // --- 与 CSIGNAL 位域重叠的标志 (仅用于 unshare/clone3) ---
        /// 创建新的 Time 命名空间。
        const NEWTIME = 0x00000080;
    }
}

/// 所有 CloneFlags 枚举中定义的有效标志
const ALL_KNOWN_FLAGS: CloneFlags = CloneFlags::all();

/// 当前内核实际支持的标志位
const CURRENTLY_SUPPORTED_FLAGS: CloneFlags = CloneFlags::from_bits_truncate(
    CloneFlags::CSIGNAL.bits()
        | CloneFlags::VM.bits()
        | CloneFlags::FS.bits()
        | CloneFlags::FILES.bits()
        | CloneFlags::SIGHAND.bits()
        | CloneFlags::PARENT.bits()
        | CloneFlags::THREAD.bits()
        | CloneFlags::PARENT_SETTID.bits()
        | CloneFlags::CHILD_SETTID.bits(),
);

impl CloneFlags {
    // 检查标志位是否在当前内核支持的范围内
    pub fn is_supported(&self) -> bool {
        self.bits() & !CURRENTLY_SUPPORTED_FLAGS.bits() == 0
    }

    /// 检查标志位是否在已知标志范围内
    pub fn is_known(&self) -> bool {
        self.bits() & !ALL_KNOWN_FLAGS.bits() == 0
    }

    /// 获取子进程退出时发送的信号编号
    pub fn get_exit_signal(&self) -> u8 {
        (self.bits() & CloneFlags::CSIGNAL.bits()) as u8
    }
}

/// 用于 clone3() 系统调用的 u64 标志位。
pub mod clone3_flags {
    pub const CLEAR_SIGHAND: u64 = 0x100000000; // 清除所有信号处理器，重置为默认值。
    pub const INTO_CGROUP: u64 = 0x200000000; // 将新任务克隆到指定的 cgroup 中。
}

/// clone3 系统调用的参数结构体。
///
/// 使用 `#[repr(C)]` 确保内存布局与 C 语言结构体兼容。
/// 字段类型使用 c_ulonglong (即 __aligned_u64) 确保 64 位对齐。
#[repr(C)]
#[derive(Debug, Default)]
pub struct CloneArgs {
    pub flags: c_ulonglong,        // 标志位
    pub pidfd: c_ulonglong,        // 存储 pidfd 的指针 (如果设置了 CLONE_PIDFD)
    pub child_tid: c_ulonglong,    // 存储子进程 TID 的指针 (如果设置了 CLONE_CHILD_SETTID)
    pub parent_tid: c_ulonglong, // 存储父进程内存中子进程 TID 的指针 (如果设置了 CLONE_PARENT_SETTID)
    pub exit_signal: c_ulonglong, // 子进程退出时发送给父进程的信号
    pub stack: c_ulonglong,      // 子进程栈的最低地址 (内核会确定栈方向)
    pub stack_size: c_ulonglong, // 子进程栈的大小
    pub tls: c_ulonglong,        // 线程局部存储描述符 (如果设置了 CLONE_SETTLS)
    pub set_tid: c_ulonglong,    // 指向 PID/TID 数组的指针 (用于多重 PID 命名空间)
    pub set_tid_size: c_ulonglong, // set_tid 数组的大小
    pub cgroup: c_ulonglong,     // cgroup 文件描述符 (如果设置了 CLONE_INTO_CGROUP)
}

// 结构体大小版本常量 (用于兼容性检查)
pub const CLONE_ARGS_SIZE_VER0: usize = 64;
pub const CLONE_ARGS_SIZE_VER1: usize = 80;
pub const CLONE_ARGS_SIZE_VER2: usize = 88;

/// Linux 系统的调度策略枚举。
#[repr(u32)] // 确保底层表示是 u32
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingPolicy {
    Normal = 0,   // SCHED_NORMAL (普通、分时调度)
    Fifo = 1,     // SCHED_FIFO (实时、先入先出)
    Rr = 2,       // SCHED_RR (实时、轮询调度)
    Batch = 3,    // SCHED_BATCH (批处理，CPU消耗型)
    Idle = 5,     // SCHED_IDLE (最低优先级，空闲时运行)
    Deadline = 6, // SCHED_DEADLINE (基于 EDF 算法的抢占式调度)
    Ext = 7,      // SCHED_EXT (保留用于外部调度器)
}

/// 调度标志：在 fork 时重置为 SCHED_NORMAL。
pub const SCHED_RESET_ON_FORK: c_int = 0x40000000;

bitflags! {
    /// 用于 sched_{set,get}attr() 系统调用的标志位。
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct SchedFlags: u32 {
        const RESET_ON_FORK = 0x01;  // 在 fork 时重置调度策略和参数。
        const RECLAIM = 0x02;        // 允许 reclaim (如用于 deadline 调度)。
        const DL_OVERRUN = 0x04;     // 允许 deadline overrun。
        const KEEP_POLICY = 0x08;    // 在 exec 时保留调度策略。
        const KEEP_PARAMS = 0x10;    // 在 exec 时保留调度参数。
        const UTIL_CLAMP_MIN = 0x20; // 设置最小利用率钳位。
        const UTIL_CLAMP_MAX = 0x40; // 设置最大利用率钳位。

        /// 保留所有策略和参数。
        const KEEP_ALL = Self::KEEP_POLICY.bits() | Self::KEEP_PARAMS.bits();

        /// 设置利用率钳位。
        const UTIL_CLAMP = Self::UTIL_CLAMP_MIN.bits() | Self::UTIL_CLAMP_MAX.bits();

        /// 所有已定义的调度标志位。
        const ALL = Self::RESET_ON_FORK.bits() |
                    Self::RECLAIM.bits() |
                    Self::DL_OVERRUN.bits() |
                    Self::KEEP_ALL.bits() |
                    Self::UTIL_CLAMP.bits();
    }
}
