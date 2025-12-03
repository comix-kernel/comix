//! fcntl 相关的用户空间 API 定义

use bitflags::bitflags;

/// fcntl 命令
///
/// 使用枚举避免魔数，提供类型安全
/// 参考：include/uapi/asm-generic/fcntl.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum FcntlCmd {
    // === 文件描述符操作 ===
    /// 复制文件描述符，新 fd >= arg (F_DUPFD)
    DupFd = 0,

    /// 获取文件描述符标志 (F_GETFD)
    GetFd = 1,

    /// 设置文件描述符标志 (F_SETFD)
    SetFd = 2,

    /// 获取文件状态标志 (F_GETFL)
    GetFl = 3,

    /// 设置文件状态标志 (F_SETFL)
    SetFl = 4,

    // === 文件锁操作 ===
    /// 获取锁信息 (F_GETLK)
    GetLk = 5,

    /// 设置锁（非阻塞）(F_SETLK)
    SetLk = 6,

    /// 设置锁（阻塞）(F_SETLKW)
    SetLkW = 7,

    // === 信号相关 ===
    /// 设置异步 I/O 所有者 (F_SETOWN)
    SetOwn = 8,

    /// 获取异步 I/O 所有者 (F_GETOWN)
    GetOwn = 9,

    /// 设置信号 (F_SETSIG)
    SetSig = 10,

    /// 获取信号 (F_GETSIG)
    GetSig = 11,

    // === 扩展命令 (Linux 特有) ===
    /// 复制 fd 并设置 CLOEXEC (F_DUPFD_CLOEXEC)
    DupFdCloexec = 1030,

    /// 设置管道大小 (F_SETPIPE_SZ)
    SetPipeSz = 1031,

    /// 获取管道大小 (F_GETPIPE_SZ)
    GetPipeSz = 1032,
}

impl FcntlCmd {
    /// 从 i32 转换为 FcntlCmd
    ///
    /// # 返回值
    /// - `Some(cmd)`: 识别的命令
    /// - `None`: 未知命令
    pub fn from_raw(cmd: i32) -> Option<Self> {
        match cmd {
            0 => Some(Self::DupFd),
            1 => Some(Self::GetFd),
            2 => Some(Self::SetFd),
            3 => Some(Self::GetFl),
            4 => Some(Self::SetFl),
            5 => Some(Self::GetLk),
            6 => Some(Self::SetLk),
            7 => Some(Self::SetLkW),
            8 => Some(Self::SetOwn),
            9 => Some(Self::GetOwn),
            10 => Some(Self::SetSig),
            11 => Some(Self::GetSig),
            1030 => Some(Self::DupFdCloexec),
            1031 => Some(Self::SetPipeSz),
            1032 => Some(Self::GetPipeSz),
            _ => None,
        }
    }
}

bitflags! {
    /// 文件描述符标志
    ///
    /// 用于 F_GETFD/F_SETFD
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FdFlags: u32 {
        /// close-on-exec 标志 (FD_CLOEXEC)
        const CLOEXEC = 1;
    }
}

bitflags! {
    /// 文件状态标志
    ///
    /// 可通过 F_SETFL 修改的标志
    /// 参考：include/uapi/asm-generic/fcntl.h
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FileStatusFlags: u32 {
        /// 追加模式 (O_APPEND)
        const APPEND = 0o2000;

        /// 非阻塞模式 (O_NONBLOCK)
        const NONBLOCK = 0o4000;

        /// 异步 I/O - 信号驱动 (O_ASYNC)
        const ASYNC = 0o20000;

        /// 直接 I/O - 绕过缓存 (O_DIRECT)
        const DIRECT = 0o40000;

        /// 不更新访问时间 (O_NOATIME)
        const NOATIME = 0o1000000;
    }
}

impl FileStatusFlags {
    /// 检查标志是否可以通过 F_SETFL 修改
    ///
    /// F_SETFL 只能修改特定标志，不能修改访问模式等
    pub fn is_modifiable(self) -> bool {
        // 只有这些标志可以被 F_SETFL 修改
        let modifiable = Self::APPEND | Self::NONBLOCK | Self::ASYNC | Self::DIRECT | Self::NOATIME;
        (self & !modifiable).is_empty()
    }
}

/// 锁类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i16)]
pub enum LockType {
    /// 共享或读锁 (F_RDLCK)
    Read = 0,

    /// 独占或写锁 (F_WRLCK)
    Write = 1,

    /// 解锁 (F_UNLCK)
    Unlock = 2,
}

impl LockType {
    pub fn from_raw(val: i16) -> Option<Self> {
        match val {
            0 => Some(Self::Read),
            1 => Some(Self::Write),
            2 => Some(Self::Unlock),
            _ => None,
        }
    }
}

/// 文件锁结构（对应 POSIX struct flock）
///
/// 用于 F_GETLK / F_SETLK / F_SETLKW 系统调用
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Flock {
    /// 锁类型：F_RDLCK, F_WRLCK, F_UNLCK
    pub l_type: i16,

    /// 起始偏移的参考位置：SEEK_SET, SEEK_CUR, SEEK_END
    pub l_whence: i16,

    /// 锁的起始偏移量
    pub l_start: i64,

    /// 锁的长度（0 表示到文件末尾）
    pub l_len: i64,

    /// 持有锁的进程 PID（仅用于 F_GETLK）
    pub l_pid: i32,

    /// 填充以保持 C 结构对齐
    _pad: i32,
}

impl Flock {
    /// 创建新的锁结构
    pub fn new(lock_type: LockType, whence: i16, start: i64, len: i64) -> Self {
        Self {
            l_type: lock_type as i16,
            l_whence: whence,
            l_start: start,
            l_len: len,
            l_pid: 0,
            _pad: 0,
        }
    }

    /// 将相对偏移转换为绝对偏移
    ///
    /// # 参数
    /// - `current_offset`: 当前文件偏移量（用于 SEEK_CUR）
    /// - `file_size`: 文件大小（用于 SEEK_END）
    pub fn to_absolute_range(
        &self,
        current_offset: usize,
        file_size: usize,
    ) -> Result<(usize, usize), ()> {
        let start = match self.l_whence as i32 {
            SEEK_SET => self.l_start,
            SEEK_CUR => current_offset as i64 + self.l_start,
            SEEK_END => file_size as i64 + self.l_start,
            _ => return Err(()),
        };

        if start < 0 {
            return Err(());
        }

        let start = start as usize;
        let len = if self.l_len == 0 {
            // 0 表示锁定到文件末尾
            usize::MAX - start
        } else if self.l_len > 0 {
            self.l_len as usize
        } else {
            return Err(());
        };

        Ok((start, len))
    }
}

bitflags! {
    /// 文件打开标志（与 POSIX 兼容）
    ///
    /// 用于 open()/openat() 系统调用
    /// 参考：include/uapi/asm-generic/fcntl.h
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct OpenFlags: u32 {
        /// 只读模式 (O_RDONLY)
        const O_RDONLY    = 0o0;

        /// 只写模式 (O_WRONLY)
        const O_WRONLY    = 0o1;

        /// 读写模式 (O_RDWR)
        const O_RDWR      = 0o2;

        /// 访问模式掩码 (O_ACCMODE)
        const O_ACCMODE   = 0o3;

        /// 文件不存在则创建 (O_CREAT)
        const O_CREAT     = 0o100;

        /// 与 O_CREAT 配合，文件必须不存在 (O_EXCL)
        const O_EXCL      = 0o200;

        /// 截断文件到 0 (O_TRUNC)
        const O_TRUNC     = 0o1000;

        /// 追加模式 (O_APPEND)
        const O_APPEND    = 0o2000;

        /// 非阻塞 I/O (O_NONBLOCK)
        const O_NONBLOCK  = 0o4000;

        /// 必须是目录 (O_DIRECTORY)
        const O_DIRECTORY = 0o200000;

        /// exec 时关闭 (O_CLOEXEC)
        const O_CLOEXEC   = 0o2000000;

        /// 大文件 (O_LARGEFILE) (空操作)
        const O_LARGEFILE = 0o100000;
    }
}

impl OpenFlags {
    /// 检查是否可读（O_RDONLY 或 O_RDWR）
    pub fn readable(&self) -> bool {
        let mode = self.bits() & Self::O_ACCMODE.bits();
        mode == Self::O_RDONLY.bits() || mode == Self::O_RDWR.bits()
    }

    /// 检查是否可写（O_WRONLY 或 O_RDWR）
    pub fn writable(&self) -> bool {
        let mode = self.bits() & Self::O_ACCMODE.bits();
        mode == Self::O_WRONLY.bits() || mode == Self::O_RDWR.bits()
    }
}

/// 文件偏移量设置模式
///
/// 用于 lseek() 系统调用
/// 对应 POSIX 的 `SEEK_SET`、`SEEK_CUR`、`SEEK_END`
/// 参考：include/uapi/linux/fs.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SeekWhence {
    /// 从文件开头计算 (SEEK_SET)
    Set = 0,

    /// 从当前位置计算 (SEEK_CUR)
    Cur = 1,

    /// 从文件末尾计算 (SEEK_END)
    End = 2,
}

impl SeekWhence {
    /// 从 i32 转换（用于系统调用参数解析）
    ///
    /// # 参数
    /// - `value`: 用户空间传入的 whence 值（0/1/2）
    ///
    /// # 返回值
    /// - `Some(whence)`: 有效的 whence 值
    /// - `None`: 无效值
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Set),
            1 => Some(Self::Cur),
            2 => Some(Self::End),
            _ => None,
        }
    }

    /// 从 usize 转换（兼容旧代码）
    pub fn from_usize(value: usize) -> Option<Self> {
        Self::from_i32(value as i32)
    }
}

// 兼容性常量（供 C 代码和文档参考）
pub const SEEK_SET: i32 = 0;
pub const SEEK_CUR: i32 = 1;
pub const SEEK_END: i32 = 2;
