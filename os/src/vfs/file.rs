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
//! - [`RegFile`](crate::vfs::RegFile) - 普通文件（Regular File），基于 Inode，支持 seek
//! - [`PipeFile`](crate::vfs::PipeFile) - 管道，流式设备，不支持 seek
//! - [`StdinFile`](crate::vfs::StdinFile) / [`StdoutFile`](crate::vfs::StdoutFile) - 标准 I/O

use crate::uapi::fcntl::{OpenFlags, SeekWhence};
use crate::vfs::{Dentry, FsError, Inode, InodeMetadata};
use alloc::sync::Arc;

/// 文件操作的统一接口
///
/// 所有打开的文件以 `Arc<dyn File>` 形式存储在进程的文件描述符表中。
///
/// # 设计要点
///
/// - 方法不携带 offset 参数，由实现者内部维护
/// - 支持可 seek 文件（RegFile）和流式设备（PipeFile）
/// - 可选方法提供默认实现（如 `lseek` 默认返回 `NotSupported`）
pub trait File: Send + Sync {
    /// 检查文件是否可读
    fn readable(&self) -> bool;

    /// 检查文件是否可写
    fn writable(&self) -> bool;

    /// 从文件读取数据
    ///
    /// RegFile 从当前 offset 读取并更新 offset；
    /// PipeFile 从管道缓冲区读取，无 offset 概念。
    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError>;

    /// 向文件写入数据
    ///
    /// RegFile 在 `O_APPEND` 模式下总是写到文件末尾。
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

    /// 获取目录项（可选方法）
    ///
    /// 默认返回`FsError::NotSupported`,适用于RegFile
    fn dentry(&self) -> Result<Arc<Dentry>, FsError> {
        Err(FsError::NotSupported)
    }

    /// 获取Inode（可选方法）
    ///
    /// 默认返回`FsError::NotSupported`,适用于RegFile
    fn inode(&self) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotSupported)
    }

    /// 设置文件状态标志（可选方法，用于 F_SETFL）
    ///
    /// 默认返回 `NotSupported`，适用于不支持动态修改标志的文件类型
    fn set_status_flags(&self, _flags: OpenFlags) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    /// 获取管道大小（可选方法，用于 F_GETPIPE_SZ）
    ///
    /// 默认返回 `NotSupported`，仅适用于 PipeFile
    fn get_pipe_size(&self) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }

    /// 设置管道大小（可选方法，用于 F_SETPIPE_SZ）
    ///
    /// 默认返回 `NotSupported`，仅适用于 PipeFile
    fn set_pipe_size(&self, _size: usize) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    /// 获取异步 I/O 所有者（可选方法，用于 F_GETOWN）
    ///
    /// 返回接收 SIGIO 信号的进程 PID
    /// 默认返回 `NotSupported`
    fn get_owner(&self) -> Result<i32, FsError> {
        Err(FsError::NotSupported)
    }

    /// 设置异步 I/O 所有者（可选方法，用于 F_SETOWN）
    ///
    /// 设置接收 SIGIO 信号的进程 PID
    /// 默认返回 `NotSupported`
    fn set_owner(&self, _pid: i32) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    /// 从指定位置读取数据（可选方法，用于 pread64/preadv）
    ///
    /// 不改变文件偏移量，默认返回 `NotSupported`，适用于非 seekable 文件
    fn read_at(&self, _offset: usize, _buf: &mut [u8]) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }

    /// 向指定位置写入数据（可选方法，用于 pwrite64/pwritev）
    ///
    /// 不改变文件偏移量，默认返回 `NotSupported`，适用于非 seekable 文件
    fn write_at(&self, _offset: usize, _buf: &[u8]) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }
}
