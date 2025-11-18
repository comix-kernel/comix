//! 文件抽象层 - VFS 会话层接口
//!
//! 定义了统一的文件操作接口 [`File`] trait，支持普通文件、管道、字符设备等多种文件类型。
//!
//! # 架构
//!
//! VFS 采用两层设计：
//! - **会话层**: [`File`] trait - 维护会话状态（offset、flags）
//! - **存储层**: [`Inode`](crate::vfs::Inode) trait - 提供无状态的随机访问
//!
//! # 实现类型
//!
//! - [`DiskFile`](crate::vfs::DiskFile) - 基于 Inode 的磁盘文件，支持 seek
//! - [`PipeFile`](crate::vfs::PipeFile) - 管道，流式设备，不支持 seek
//! - [`StdinFile`](crate::vfs::StdinFile) / [`StdoutFile`](crate::vfs::StdoutFile) - 标准 I/O

use crate::vfs::{FsError, InodeMetadata};

/// 文件操作的统一接口
///
/// 所有打开的文件以 `Arc<dyn File>` 形式存储在进程的文件描述符表中。
///
/// # 设计要点
///
/// - 方法不携带 offset 参数，由实现者内部维护
/// - 支持可 seek 文件（DiskFile）和流式设备（PipeFile）
/// - 可选方法提供默认实现（如 `lseek` 默认返回 `NotSupported`）
pub trait File: Send + Sync {
    /// 检查文件是否可读
    fn readable(&self) -> bool;

    /// 检查文件是否可写
    fn writable(&self) -> bool;

    /// 从文件读取数据
    ///
    /// DiskFile 从当前 offset 读取并更新 offset；
    /// PipeFile 从管道缓冲区读取，无 offset 概念。
    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError>;

    /// 向文件写入数据
    ///
    /// DiskFile 在 `O_APPEND` 模式下总是写到文件末尾。
    fn write(&self, buf: &[u8]) -> Result<usize, FsError>;

    /// 获取文件元数据
    fn metadata(&self) -> Result<InodeMetadata, FsError>;

    /// 设置文件偏移量（可选方法）
    ///
    /// 默认返回 `NotSupported`，适用于流式设备。
    fn lseek(&self, _offset: isize, _whence: SeekWhence) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }

    /// 获取当前偏移量（可选方法）
    ///
    /// 默认返回 0，适用于流式设备。
    fn offset(&self) -> usize {
        0
    }

    /// 获取打开标志（可选方法）
    ///
    /// 用于 exec 时关闭 `O_CLOEXEC` 文件。
    fn flags(&self) -> OpenFlags {
        OpenFlags::empty()
    }
}

/// 文件偏移量设置模式
///
/// 对应 POSIX 的 `SEEK_SET`、`SEEK_CUR`、`SEEK_END`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum SeekWhence {
    /// 从文件开头计算
    Set = 0,
    /// 从当前位置计算
    Cur = 1,
    /// 从文件末尾计算
    End = 2,
}

impl SeekWhence {
    /// 从 usize 转换（0=Set, 1=Cur, 2=End）
    pub fn from_usize(value: usize) -> Option<Self> {
        match value {
            0 => Some(SeekWhence::Set),
            1 => Some(SeekWhence::Cur),
            2 => Some(SeekWhence::End),
            _ => None,
        }
    }
}

bitflags::bitflags! {
    /// 文件打开标志（与 POSIX 兼容）
    #[derive(Clone)]
    pub struct OpenFlags: u32 {
        const O_RDONLY    = 0o0;        // 只读
        const O_WRONLY    = 0o1;        // 只写
        const O_RDWR      = 0o2;        // 读写
        const O_ACCMODE   = 0o3;        // 访问模式掩码
        const O_CREAT     = 0o100;      // 不存在则创建
        const O_EXCL      = 0o200;      // 与 O_CREAT 配合，必须不存在
        const O_TRUNC     = 0o1000;     // 截断到 0
        const O_APPEND    = 0o2000;     // 追加模式
        const O_NONBLOCK  = 0o4000;     // 非阻塞 I/O
        const O_DIRECTORY = 0o200000;   // 必须是目录
        const O_CLOEXEC   = 0o2000000;  // exec 时关闭
    }
}

impl OpenFlags {
    /// 检查是否可读（O_RDONLY 或 O_RDWR）
    pub fn readable(&self) -> bool {
        let mode = self.bits() & OpenFlags::O_ACCMODE.bits();
        mode == OpenFlags::O_RDONLY.bits() || mode == OpenFlags::O_RDWR.bits()
    }

    /// 检查是否可写（O_WRONLY 或 O_RDWR）
    pub fn writable(&self) -> bool {
        let mode = self.bits() & OpenFlags::O_ACCMODE.bits();
        mode == OpenFlags::O_WRONLY.bits() || mode == OpenFlags::O_RDWR.bits()
    }
}
