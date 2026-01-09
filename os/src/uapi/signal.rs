//! 信号处理行为标志和相关常量定义
//!
//! 这些常量用于信号处理函数的行为控制，
//! 以及信号屏蔽操作等。

use core::{
    ffi::{c_char, c_int, c_long, c_uint, c_ulong, c_ulonglong, c_void},
    fmt::Debug,
};

use bitflags::bitflags;

use crate::{
    arch::trap::TrapFrame,
    uapi::types::{ClockT, PidT, SigSetT, SizeT, UidT},
};

/// 信号集合的大小（以字节为单位）
pub const SIGSET_SIZE: usize = core::mem::size_of::<SigSetT>();

#[repr(C)]
#[derive(Clone, Copy)]
pub union __SaHandler {
    /// C: `void (*sa_handler)(int)`
    /// 用于 SA_SIGINFO 未设置时的普通处理器 (单参数)。
    pub sa_handler: SaHandlerPtr,

    /// C: `void (*sa_sigaction)(int, siginfo_t *, void *)`
    /// 用于 SA_SIGINFO 设置时的实时信号处理器 (三参数)。
    pub sa_sigaction: SaSigactionPtr,
}

impl Debug for __SaHandler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "__SaHandler {{ ... }}")
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
/// Linux/POSIX 信号处理动作结构体 (struct sigaction)
pub struct SignalAction {
    /// C: `union { ... } __sa_handler`
    /// 包含单参数或三参数信号处理函数指针。
    pub __sa_handler: __SaHandler,

    /// C: `unsigned long sa_flags`
    ///
    /// On 64-bit Linux, sa_flags is `unsigned long` (not `int`), and appears before
    /// sa_restorer/sa_mask in the kernel ABI.
    pub sa_flags: c_ulong,

    /// C: `void (*sa_restorer)(void)`
    /// 信号恢复函数指针。通常由 C 库设置，用于从信号处理器返回。
    pub sa_restorer: SaRestorerPtr,

    /// C: `sigset_t sa_mask`
    /// 信号屏蔽字，在执行处理函数时将被自动添加到线程的阻塞集中。
    pub sa_mask: SigSetT,
}

impl SignalAction {
    /// 获取单参数信号处理器。
    /// 这是一个不安全操作，因为它访问联合体字段。
    pub unsafe fn sa_handler(&self) -> SaHandlerPtr {
        unsafe { self.__sa_handler.sa_handler }
    }

    /// 获取三参数信号处理器。
    /// 这是一个不安全操作，因为它访问联合体字段。
    pub unsafe fn sa_sigaction(&self) -> SaSigactionPtr {
        unsafe { self.__sa_handler.sa_sigaction }
    }

    /// 安全地检查是否设置了 SA_SIGINFO 标志。
    pub fn is_siginfo(&self) -> bool {
        // 假设您的 SaFlags 已经正确转换并可以使用
        (self.sa_flags as u32) & SaFlags::SIGINFO.bits() != 0
    }

    /// 创建一个新的 SignalAction 实例。
    /// # 参数：
    /// * `handler`: 信号处理函数指针 (单参数)
    /// * `flags`: 信号处理行为标志
    /// * `mask`: 信号屏蔽字
    /// # 返回值：新的 SignalAction 实例
    pub fn new(handler: SaHandlerPtr, flags: SaFlags, mask: SignalFlags) -> Self {
        Self {
            __sa_handler: __SaHandler {
                sa_handler: handler,
            },
            sa_restorer: core::ptr::null_mut(),
            sa_flags: flags.bits() as c_ulong,
            sa_mask: mask.bits() as SigSetT,
        }
    }
}

impl Default for SignalAction {
    /// 创建一个默认的 SignalAction 实例，使用 SIG_DFL 作为处理器，空标志和空屏蔽字。
    fn default() -> Self {
        Self::new(SIG_DFL as *mut _, SaFlags::empty(), SignalFlags::empty())
    }
}

/* 信号定义 */
pub const NSIG: usize = 31;

pub const NUM_SIGHUP: usize = 1;
pub const NUM_SIGINT: usize = 2;
pub const NUM_SIGQUIT: usize = 3;
pub const NUM_SIGILL: usize = 4;
pub const NUM_SIGTRAP: usize = 5;
pub const NUM_SIGABRT: usize = 6;
pub const NUM_SIGBUS: usize = 7;
pub const NUM_SIGFPE: usize = 8;
pub const NUM_SIGKILL: usize = 9;
pub const NUM_SIGUSR1: usize = 10;
pub const NUM_SIGSEGV: usize = 11;
pub const NUM_SIGUSR2: usize = 12;
pub const NUM_SIGPIPE: usize = 13;
pub const NUM_SIGALRM: usize = 14;
pub const NUM_SIGTERM: usize = 15;
pub const NUM_SIGSTKFLT: usize = 16;
pub const NUM_SIGCHLD: usize = 17;
pub const NUM_SIGCONT: usize = 18;
pub const NUM_SIGSTOP: usize = 19;
pub const NUM_SIGTSTP: usize = 20;
pub const NUM_SIGTTIN: usize = 21;
pub const NUM_SIGTTOU: usize = 22;
pub const NUM_SIGURG: usize = 23;
pub const NUM_SIGXCPU: usize = 24;
pub const NUM_SIGXFSZ: usize = 25;
pub const NUM_SIGVTALRM: usize = 26;
pub const NUM_SIGPROF: usize = 27;
pub const NUM_SIGWINCH: usize = 28;
pub const NUM_SIGIO: usize = 29;
pub const NUM_SIGPWR: usize = 30;
pub const NUM_SIGSYS: usize = 31;

bitflags! {
    #[derive(Clone, Debug, Copy)]
    #[repr(transparent)]
    /// 信号标志位，用于表示信号集合。
    pub struct SignalFlags: usize {
        const SIGHUP = 1 << (NUM_SIGHUP - 1);
        const SIGINT = 1 << (NUM_SIGINT - 1);
        const SIGQUIT = 1 << (NUM_SIGQUIT - 1);
        const SIGILL = 1 << (NUM_SIGILL - 1);
        const SIGTRAP = 1 << (NUM_SIGTRAP - 1);
        const SIGABRT = 1 << (NUM_SIGABRT - 1);
        const SIGBUS = 1 << (NUM_SIGBUS - 1);
        const SIGFPE = 1 << (NUM_SIGFPE - 1);
        const SIGKILL = 1 << (NUM_SIGKILL - 1);
        const SIGUSR1 = 1 << (NUM_SIGUSR1 - 1);
        const SIGSEGV = 1 << (NUM_SIGSEGV - 1);
        const SIGUSR2 = 1 << (NUM_SIGUSR2 - 1);
        const SIGPIPE = 1 << (NUM_SIGPIPE - 1);
        const SIGALRM = 1 << (NUM_SIGALRM - 1);
        const SIGTERM = 1 << (NUM_SIGTERM - 1);
        const SIGSTKFLT = 1 << (NUM_SIGSTKFLT - 1);
        const SIGCHLD = 1 << (NUM_SIGCHLD - 1);
        const SIGCONT = 1 << (NUM_SIGCONT - 1);
        const SIGSTOP = 1 << (NUM_SIGSTOP - 1);
        const SIGTSTP = 1 << (NUM_SIGTSTP - 1);
        const SIGTTIN = 1 << (NUM_SIGTTIN - 1);
        const SIGTTOU = 1 << (NUM_SIGTTOU - 1);
        const SIGURG = 1 << (NUM_SIGURG - 1);
        const SIGXCPU = 1 << (NUM_SIGXCPU - 1);
        const SIGXFSZ = 1 << (NUM_SIGXFSZ - 1);
        const SIGVTALRM = 1 << (NUM_SIGVTALRM - 1);
        const SIGPROF = 1 << (NUM_SIGPROF - 1);
        const SIGWINCH = 1 << (NUM_SIGWINCH - 1);
        const SIGIO = 1 << (NUM_SIGIO - 1);
        const SIGPWR = 1 << (NUM_SIGPWR - 1);
        const SIGSYS = 1 << (NUM_SIGSYS - 1);
    }
}

impl SignalFlags {
    pub fn from_signal_num(sig_num: usize) -> Option<Self> {
        if sig_num == 0 || sig_num > NSIG {
            return None;
        }
        Some(SignalFlags::from_bits(1 << (sig_num - 1)).unwrap())
    }

    /// 将 SignalFlags 转换为对应的信号编号 (1-NSIG)。
    /// 如果包含多个信号，则返回第一个匹配的信号编号。
    /// 如果不包含任何信号，则返回 0 表示无效。
    pub fn to_signal_number(&self) -> usize {
        for sig_num in 1..=NSIG {
            if self.contains(SignalFlags::from_bits(1 << (sig_num - 1)).unwrap()) {
                return sig_num;
            }
        }
        0 // 如果没有匹配的信号，返回 0 表示无效
    }

    pub fn from_sigset_t(set: SigSetT) -> Self {
        // Userspace may pass a full-width sigset_t with reserved/high bits set.
        // For Linux ABI compatibility, ignore unknown bits instead of panicking.
        SignalFlags::from_bits_truncate(set as usize)
    }

    pub fn to_sigset_t(&self) -> SigSetT {
        self.bits() as SigSetT
    }
}

// --- 信号处理函数类型定义 ---
// C 兼容的函数指针类型
pub type SaHandlerFn = extern "C" fn(c_int);
pub type SaSigactionFn = extern "C" fn(c_int, *const SigInfoT, *mut c_void);
pub type SaRestorerFn = extern "C" fn();

// 指针别名，用于存储 SIG_DFL (0) 和 SIG_IGN (1)
pub type SaHandlerPtr = *mut SaHandlerFn;
pub type SaSigactionPtr = *mut SaSigactionFn;
pub type SaRestorerPtr = *mut SaRestorerFn;

// --- 信号处理行为标志 (SA_FLAGS) ---
// 用于 struct sigaction 的 sa_flags 字段，控制信号处理函数的行为。
bitflags! {
    /// 信号处理行为标志，用于 sigaction 系统调用。
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(transparent)]
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

        /// 使用用户提供的 sa_restorer（架构相关的历史标志）。
        ///
        /// musl 在调用 rt_sigaction 时常携带该标志；为了 Linux ABI 兼容性需要接受它。
        const RESTORER = 0x0400_0000;

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

const ALL_KNOWN_FLAGS: SaFlags = SaFlags::all();

// Best-effort: accept all known flags for Linux ABI compatibility.
// Individual semantics can be implemented incrementally.
const NOW_SUPPORTED_FLAGS: SaFlags = ALL_KNOWN_FLAGS;

impl SaFlags {
    /// 检查标志位是否在当前内核支持的范围内
    pub fn is_supported(&self) -> bool {
        self.bits() & !NOW_SUPPORTED_FLAGS.bits() == 0
    }

    /// 检查标志位是否在已知标志范围内
    pub fn is_known(&self) -> bool {
        self.bits() & !ALL_KNOWN_FLAGS.bits() == 0
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

#[repr(C)]
#[derive(Clone, Copy)]
/// 信号详细信息结构体 (siginfo_t)
pub struct SigInfoT {
    // --- 头部字段 (条件编译省略) ---
    // 这是大多数现代系统的默认顺序
    pub si_signo: c_int,
    pub si_errno: c_int,
    pub si_code: c_int,

    // --- 联合体字段 ---
    pub __si_fields: __SiFields,
}

impl SigInfoT {
    /// 创建一个新的、空的 SigInfoT 实例，所有字段初始化为零。
    pub fn new() -> Self {
        Self {
            si_signo: 0,
            si_errno: 0,
            si_code: 0,
            __si_fields: __SiFields {
                __pad: [0; __SIGINFO_PAD_SIZE],
            },
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union Sigval {
    pub sival_int: c_int,
    pub sival_ptr: *mut c_void,
}

const __SIGINFO_PAD_SIZE: usize =
    128 - 2 * core::mem::size_of::<c_int>() - core::mem::size_of::<c_long>();

#[repr(C)]
#[derive(Clone, Copy)]
pub union __SiFields {
    // char __pad[128 - 2*sizeof(int) - sizeof(long)];
    pub __pad: [c_char; __SIGINFO_PAD_SIZE], // 使用计算出的填充数组

    pub __si_common: __SiCommon,
    pub __sigfault: __SigFault,
    pub __sigpoll: __SigPoll,
    pub __sigsys: __SigSys,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct __SiCommon {
    pub __first: __FirstCommon,
    pub __second: __SecondCommon,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct __SigFault {
    pub si_addr: *mut c_void,
    pub si_addr_lsb: i16, // short
    pub __first: __FirstFault,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct __SigPoll {
    pub si_band: c_long, // long
    pub si_fd: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct __SigSys {
    pub si_call_addr: *mut c_void,
    pub si_syscall: c_int,
    pub si_arch: c_uint, // unsigned
}

// --- 内部联合体和结构体定义 (自内向外) ---

#[repr(C)]
#[derive(Clone, Copy)]
pub union __FirstCommon {
    pub __piduid: __PidUid,
    pub __timer: __Timer,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct __PidUid {
    pub si_pid: PidT,
    pub si_uid: UidT,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct __Timer {
    pub si_timerid: c_int,
    pub si_overrun: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union __SecondCommon {
    pub si_value: Sigval,
    pub __sigchld: __SigChld,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct __SigChld {
    pub si_status: c_int,
    pub si_utime: ClockT,
    pub si_stime: ClockT,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union __FirstFault {
    pub __addr_bnd: __AddrBnd,
    pub si_pkey: c_uint, // unsigned si_pkey
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct __AddrBnd {
    pub si_lower: *mut c_void,
    pub si_upper: *mut c_void,
}

/// 用户态信号处理上下文结构体
/// 该结构体包含恢复进程执行所需的所有状态。
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct UContextT {
    /// 标志位
    pub uc_flags: c_ulong,
    /// 链接到下一个上下文
    pub uc_link: *mut UContextT,
    /// 信号栈信息
    pub uc_stack: SignalStack,
    /// 信号掩码
    pub uc_sigmask: SigSetT,
    /// 机器上下文
    pub uc_mcontext: MContextT,
}

/// Linux rt_sigreturn frame layout (rt_sigframe):
/// userspace restorer expects siginfo followed by ucontext on the stack.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RtSigFrame {
    pub info: SigInfoT,
    pub uc: UContextT,
}

impl UContextT {
    /// 创建一个新的 UContextT 实例，所有字段初始化为零或默认值。
    pub fn default() -> Self {
        Self {
            uc_flags: 0,
            uc_link: core::ptr::null_mut(),
            uc_stack: SignalStack::default(),
            uc_sigmask: 0,
            uc_mcontext: MContextT::new(),
        }
    }

    /// 创建一个新的 UContextT 实例，使用指定的字段值。
    /// # 参数:
    /// * `flags`: 上下文标志
    /// * `link`: 指向下一个上下文的指针
    /// * `stack`: 信号栈信息
    /// * `sigmask`: 信号掩码
    /// * `mcontext`: 机器上下文
    /// # 返回值: 新的 UContextT 实例
    pub fn new(
        flags: c_ulong,
        link: *mut UContextT,
        stack: SignalStack,
        sigmask: SigSetT,
        mcontext: MContextT,
    ) -> Self {
        Self {
            uc_flags: flags,
            uc_link: link,
            uc_stack: stack,
            uc_sigmask: sigmask,
            uc_mcontext: mcontext,
        }
    }
}

/// 信号栈信息结构体
/// 该结构体用于描述备用信号栈的信息。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SignalStack {
    /// 栈顶地址, 对应 C 中的 void *ss_sp
    pub ss_sp: usize,
    /// 栈状态标志
    pub ss_flags: c_int,
    /// 栈大小
    pub ss_size: SizeT,
}

/// 最小信号栈大小
pub const MINSIGSTKSZ: usize = 2048;
/// 默认信号栈大小
pub const SIGSTKSZ: usize = 8192;
// 信号栈标志位
/// 信号栈正在使用中
pub const SS_ONSTACK: usize = 1;
/// 信号栈被禁用
pub const SS_DISABLE: usize = 2;
/// 自动解除信号栈
pub const SS_AUTODISARM: usize = 1 << 31;
/// 信号栈标志位掩码
pub const SS_FLAG_BITS: usize = SS_AUTODISARM;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
/// 机器上下文结构体
pub struct MContextT {
    /// 通用寄存器数组
    pub gregs: [c_ulong; 32],
    /// 浮点寄存器数组
    /// XXX: 实际并未使用浮点寄存器
    ///      且注意struct pending?
    pub fpregs: [c_ulonglong; 66],
}

impl MContextT {
    /// 创建一个新的 MContextT 实例，所有寄存器初始化为零。
    pub fn new() -> Self {
        Self {
            gregs: [0; 32],
            fpregs: [0; 66],
        }
    }

    /// 从 TrapFrame 创建 MContextT 实例
    pub fn from_trap_frame(tf: &TrapFrame) -> Self {
        tf.to_mcontext()
    }
}

/// uc_mcontext_ext has valid high gprs
pub const UC_GPRS_HIGH: usize = 1;
/// uc_mcontext_ext has valid vector regs
pub const UC_VXRS: usize = 2;
