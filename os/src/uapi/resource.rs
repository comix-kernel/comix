//! 资源限制相关的常量和类型定义。

use core::{ffi::c_long, usize};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
/// 进程或其子进程的资源使用统计。
pub struct Rusage {
    /// ru_utime: 用户模式下消耗的 CPU 时间。
    pub ru_utime: timeval,

    /// ru_stime: 内核模式下消耗的 CPU 时间。
    pub ru_stime: timeval,

    /// ru_maxrss: 最大常驻内存集大小 (Maximum Resident Set Size)，单位：千字节 (KB)。
    pub ru_maxrss: c_long,

    /// ru_ixrss: 积分共享内存大小 (Integral Shared Memory Size)。
    pub ru_ixrss: c_long,

    /// ru_idrss: 积分非共享数据大小 (Integral Unshared Data Size)。
    pub ru_idrss: c_long,

    /// ru_isrss: 积分非共享栈大小 (Integral Unshared Stack Size)。
    pub ru_isrss: c_long,

    /// ru_minflt: 页回收次数 (Page Reclaims)，即次要页错误 (Minor Faults)。
    pub ru_minflt: c_long,

    /// ru_majflt: 页错误次数 (Page Faults)，即主要页错误 (Major Faults)。
    pub ru_majflt: c_long,

    /// ru_nswap: 交换次数 (Swaps)。
    pub ru_nswap: c_long,

    /// ru_inblock: 块输入操作次数 (Block Input Operations)。
    pub ru_inblock: c_long,

    /// ru_oublock: 块输出操作次数 (Block Output Operations)。
    pub ru_oublock: c_long,

    /// ru_msgsnd: 发送的消息次数 (Messages Sent)。
    pub ru_msgsnd: c_long,

    /// ru_msgrcv: 接收的消息次数 (Messages Received)。
    pub ru_msgrcv: c_long,

    /// ru_nsignals: 接收到的信号次数 (Signals Received)。
    pub ru_nsignals: c_long,

    /// ru_nvcsw: 自愿上下文切换次数 (Voluntary Context Switches)。
    pub ru_nvcsw: c_long,

    /// ru_nivcsw: 非自愿上下文切换次数 (Involuntary Context Switches)。
    pub ru_nivcsw: c_long,
}

/// 资源限制值相关的定义。
pub mod rlimit_value {
    /// 资源限制值类型，对应 C 语言中的 rlim_t，通常是 usize (64位无符号长整型)。
    pub type RlimT = usize;

    /// 表示资源无限制（无穷大）的值。
    pub const RLIM_INFINITY: RlimT = usize::MAX;

    // --- 默认资源限制值常量 ---
    /// 栈的默认软限制：8MB。
    pub const STACK_DEFAULT_LIMIT: RlimT = 8 * 1024 * 1024;
    /// 默认文件描述符软限制（Current）。
    pub const FILE_OPEN_CUR_DEFAULT: RlimT = 1024;
    /// 默认文件描述符硬限制（Maximum）。
    pub const FILE_OPEN_MAX_DEFAULT: RlimT = 4096;
    /// 内存锁定限制的默认值：64KB。
    pub const MEMLOCK_DEFAULT_LIMIT: RlimT = 64 * 1024;
    /// 消息队列的最大字节数默认值：800KB。
    pub const MQ_BYTES_MAX_DEFAULT: RlimT = 819200;
}

use rlimit_value::*;

use crate::uapi::time::timeval;

/// 资源限制 ID (Resource limit IDs)
/// 使用枚举封装 RLIMIT_* 宏，强制类型安全。
/// #[repr(i32)] 确保底层存储与 C 语言的整数 ID 兼容。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ResourceId {
    /// CPU 时间限制（秒）。
    Cpu = 0,
    /// 文件最大尺寸限制。
    Fsize = 1,
    /// 数据段最大尺寸限制。
    Data = 2,
    /// 栈最大尺寸限制。
    Stack = 3,
    /// core dump 文件最大尺寸限制。
    Core = 4,

    /// 最大驻留集大小（Max Resident Set Size）。
    Rss = 5,
    /// 用户最大进程数限制。
    Nproc = 6,
    /// 最大打开文件描述符数量限制。
    Nofile = 7,
    /// 锁定内存地址空间的最大限制（字节）。
    Memlock = 8,
    /// 进程虚拟地址空间的最大限制（字节）。
    As = 9,

    /// 最大文件锁数量限制。
    Locks = 10,
    /// 最大待处理信号数量限制。
    Sigpending = 11,
    /// POSIX 消息队列的最大字节数限制。
    Msgqueue = 12,
    /// 可设置的 nice 优先级上限。
    Nice = 13,
    /// 最大实时优先级限制。
    Rtprio = 14,
    /// 实时任务的超时时间限制（微秒）。
    Rttime = 15,
}

/// 资源限制结构体，对应 C 语言的 struct rlimit。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Rlimit {
    /// 软限制（内核实际强制执行的值）。
    pub rlim_cur: RlimT,
    /// 硬限制（软限制的上限）。
    pub rlim_max: RlimT,
}

impl Rlimit {
    /// 构造一个新的 Rlimit 实例。
    pub const fn new(rlim_cur: RlimT, rlim_max: RlimT) -> Self {
        Rlimit { rlim_cur, rlim_max }
    }

    /// 构造一个无限限制的 Rlimit 实例。
    pub const fn inf() -> Self {
        Rlimit {
            rlim_cur: RLIM_INFINITY,
            rlim_max: RLIM_INFINITY,
        }
    }

    /// 构造一个默认的 Rlimit 实例，软硬限制均为 0。
    pub const fn default() -> Self {
        Rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        }
    }
}

/// 资源限制的总数量。
pub const RLIM_NLIMITS: usize = 16;

/// 启动时 init 任务的默认资源限制数组。
pub const INIT_RLIMITS: [Rlimit; RLIM_NLIMITS] = {
    let mut limits = [Rlimit::default(); RLIM_NLIMITS];

    const fn id_to_index(id: ResourceId) -> usize {
        id as u32 as usize
    }

    limits[id_to_index(ResourceId::Cpu)] = Rlimit::inf();
    limits[id_to_index(ResourceId::Fsize)] = Rlimit::inf();
    limits[id_to_index(ResourceId::Data)] = Rlimit::inf();
    limits[id_to_index(ResourceId::Stack)] = Rlimit::new(STACK_DEFAULT_LIMIT, RLIM_INFINITY);
    limits[id_to_index(ResourceId::Core)] = Rlimit::new(0, RLIM_INFINITY);
    limits[id_to_index(ResourceId::Rss)] = Rlimit::inf();
    limits[id_to_index(ResourceId::Nproc)] = Rlimit::new(0, 0);
    limits[id_to_index(ResourceId::Nofile)] =
        Rlimit::new(FILE_OPEN_CUR_DEFAULT, FILE_OPEN_MAX_DEFAULT);
    limits[id_to_index(ResourceId::Memlock)] =
        Rlimit::new(MEMLOCK_DEFAULT_LIMIT, MEMLOCK_DEFAULT_LIMIT);
    limits[id_to_index(ResourceId::As)] = Rlimit::inf();
    limits[id_to_index(ResourceId::Locks)] = Rlimit::inf();
    limits[id_to_index(ResourceId::Sigpending)] = Rlimit::new(0, 0);
    limits[id_to_index(ResourceId::Msgqueue)] =
        Rlimit::new(MQ_BYTES_MAX_DEFAULT, MQ_BYTES_MAX_DEFAULT);
    limits[id_to_index(ResourceId::Nice)] = Rlimit::new(0, 0);
    limits[id_to_index(ResourceId::Rtprio)] = Rlimit::new(0, 0);
    limits[id_to_index(ResourceId::Rttime)] = Rlimit::inf();
    limits
};

/// 资源限制结构体数组
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RlimitStruct {
    pub limits: [Rlimit; RLIM_NLIMITS],
}

impl RlimitStruct {
    /// 构造一个新的 RlimitStruct 实例。
    pub const fn new(limits: [Rlimit; RLIM_NLIMITS]) -> Self {
        RlimitStruct { limits }
    }
}
