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

    /// 获取目录项（可选方法）
    ///
    /// 默认返回`FsError::NotSupported`,适用于DiskFile
    fn dentry(&self) -> Result<Arc<Dentry>, FsError> {
        Err(FsError::NotSupported)
    }

    /// 获取Inode（可选方法）
    ///
    /// 默认返回`FsError::NotSupported`,适用于DiskFile
    fn inode(&self) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotSupported)
    }
}
