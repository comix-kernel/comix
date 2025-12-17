//! 文件抽象层 - VFS 会话层接口
//!
//! 该模块定义了统一的文件操作接口 [`File`] trait，支持普通文件、管道、字符设备等多种文件类型。
//! 所有打开的文件以 `Arc<dyn File>` 形式存储在进程的文件描述符表中。
//!
//! # 架构定位
//!
//! VFS 采用两层设计：
//! - **会话层**: [`File`] trait - 维护会话状态（offset、flags），有状态操作
//! - **存储层**: [`Inode`] trait - 提供无状态的随机访问
//!
//! ## 为什么分离会话层和存储层?
//!
//! 同一个文件可以被多次打开或 dup，每次打开都需要独立的会话状态（如 offset），
//! 但它们共享相同的底层存储（Inode）。这种设计支持：
//!
//! - **dup 语义**: 复制的文件描述符共享 offset
//! - **硬链接**: 多个路径指向同一个 Inode
//! - **fork 继承**: 父子进程共享文件表
//!
//! ```text
//! 进程 A: fd[3] ──┐
//!                 ├──> Arc<RegFile> { offset: 100 }
//! 进程 A: fd[4] ──┘          │
//!                            ▼
//!                       Arc<Dentry>
//!                            │
//!                            ▼
//!                       Arc<Inode> ←─── 进程 B: fd[5] -> Arc<RegFile> { offset: 200 }
//! ```
//!
//! # 实现类型
//!
//! - [`RegFile`](crate::vfs::RegFile) - 普通文件，基于 Inode，支持 seek
//! - [`PipeFile`](crate::vfs::PipeFile) - 管道，环形缓冲区，流式设备
//! - [`StdinFile`](crate::vfs::StdinFile) / [`StdoutFile`](crate::vfs::StdoutFile) / [`StderrFile`](crate::vfs::StderrFile) - 标准 I/O
//! - `CharDevFile` - 字符设备文件（串口、终端等）
//! - `BlkDevFile` - 块设备文件（磁盘等）
//!
//! # 设计特点
//!
//! ## 可选方法
//!
//! File trait 中的许多方法提供了默认实现（返回 `NotSupported`），
//! 允许不同文件类型只实现自己支持的功能：
//!
//! - `lseek()`: 仅 RegFile 和 BlkDevFile 支持
//! - `get_pipe_size()`: 仅 PipeFile 支持
//! - `ioctl()`: 仅设备文件支持
//!
//! ## 原子操作
//!
//! RegFile 使用 `AtomicUsize` 管理 offset，支持无锁并发读写：
//!
//! ```text
//! 线程 1: read() -> fetch_add(n)
//! 线程 2: read() -> fetch_add(m)  // 并发安全
//! ```
//!
//! # 使用示例
//!
//! ```rust
//! use vfs::{vfs_lookup, RegFile, OpenFlags, File};
//! use alloc::sync::Arc;
//!
//! // 1. 创建 File 对象
//! let dentry = vfs_lookup("/etc/passwd")?;
//! let file: Arc<dyn File> = Arc::new(
//!     RegFile::new(dentry, OpenFlags::O_RDONLY)
//! );
//!
//! // 2. 读取数据
//! let mut buf = [0u8; 1024];
//! let n = file.read(&mut buf)?;
//!
//! // 3. 检查文件属性
//! assert!(file.readable());
//! assert!(!file.writable());
//!
//! // 4. 获取元数据
//! let metadata = file.metadata()?;
//! println!("文件大小: {}", metadata.size);
//! ```

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

    /// 执行设备特定的控制操作（可选方法，用于 ioctl）
    ///
    /// 用于设备驱动程序特定的控制命令。
    /// - `request`: ioctl 请求码
    /// - `arg`: 参数指针（作为 usize 传递）
    ///
    /// 默认返回 `NotSupported`，仅由支持 ioctl 的设备类型实现
    fn ioctl(&self, _request: u32, _arg: usize) -> Result<isize, FsError> {
        Err(FsError::NotSupported)
    }

    /// 获取 Any trait 引用，用于安全的类型转换
    fn as_any(&self) -> &dyn core::any::Any;

    /// 从socket接收数据并获取源地址（可选方法，用于recvfrom）
    ///
    /// 返回(读取字节数, 源地址)
    /// 默认返回NotSupported，仅socket实现
    fn recvfrom(&self, _buf: &mut [u8]) -> Result<(usize, Option<alloc::vec::Vec<u8>>), FsError> {
        Err(FsError::NotSupported)
    }
}
