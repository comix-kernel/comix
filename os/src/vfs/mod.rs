//! 虚拟文件系统（VFS）层
//!
//! 该模块提供了一个 **POSIX 兼容的虚拟文件系统抽象**，支持多种文件类型和文件系统的统一访问。
//!
//! # 组件
//!
//! - [`mod@file`] - 会话层接口定义 (File trait)
//! - [`inode`] - 存储层接口定义 (Inode trait)
//! - [`dentry`] - 目录项结构和全局缓存
//! - [`path`] - 路径解析引擎（绝对/相对路径、符号链接）
//! - [`mount`] - 挂载表管理和挂载点栈
//! - [`fd_table`] - 进程级文件描述符表
//! - [`file_lock`] - POSIX 文件锁管理器
//! - [`file_system`] - 文件系统抽象接口
//! - [`impls`] - 具体文件类型实现（RegFile、PipeFile 等）
//! - [`error`] - VFS 错误类型定义
//! - [`dev`]/[`devno`] - 设备号管理和驱动注册
//!
//! # 设计概览
//!
//! ## 分层架构
//!
//! VFS 采用四层架构设计，从上到下依次为：
//!
//! 1. **应用层**：系统调用接口和文件描述符表 ([`FDTable`])
//! 2. **会话层**：有状态的文件操作接口 ([`File`] trait)，维护 offset 和 flags
//! 3. **路径层**：目录树管理和路径解析 ([`Dentry`]、[`path`] 模块)
//! 4. **存储层**：无状态的存储访问接口 ([`Inode`] trait)
//!
//! ## 核心设计理念
//!
//! ### 分离会话与存储
//!
//! - **会话层 (File)**：每次 `open()` 创建新的 `File` 实例，维护独立的 `offset` 和 `flags`
//! - **存储层 (Inode)**：多个 `File` 可共享同一个 `Inode`，实现硬链接和 dup 语义
//!
//! ```text
//! fd[3] ──┐
//!         ├──> Arc<RegFile> { offset: 100, ... }
//! fd[4] ──┘                     │
//!                               │
//!                               ▼
//!                          Arc<Dentry>
//!                               │
//!                               ▼
//!                          Arc<dyn Inode>
//! ```
//!
//! ### 目录项缓存
//!
//! - **全局缓存** ([`DENTRY_CACHE`])：路径 → `Weak<Dentry>` 映射，避免重复解析
//! - **树状缓存**：Dentry 内部维护父子关系，加速相对路径查找
//! - **自动失效**：使用 `Weak` 引用，不再使用的 Dentry 自动回收
//!
//! ### 挂载表管理
//!
//! - **最长前缀匹配**：访问 `/mnt/data/file` 时自动选择最匹配的挂载点
//! - **挂载点栈**：同一路径可多次挂载，最后挂载的文件系统覆盖前面的
//! - **全局单例** ([`MOUNT_TABLE`])：所有挂载点的集中管理
//!
//! ## 性能特点
//!
//! - **零拷贝读写**：RegFile 直接调用 Inode 的 `read_at`/`write_at`，无额外拷贝
//! - **原子偏移量**：使用 `AtomicUsize` 管理文件偏移，无锁并发读写
//! - **多级缓存**：Dentry 缓存、挂载点缓存减少重复查找
//! - **引用计数**：使用 `Arc`/`Weak` 自动管理对象生命周期，无需手动释放
//!
//! ## 并发安全
//!
//! - **FDTable**：内部使用 `SpinLock` 保护文件描述符数组
//! - **DentryCache**：使用 `SpinLock` 保护全局缓存
//! - **MountTable**：使用 `SpinLock` 保护挂载表
//! - **FileLockManager**：使用 `SpinLock` 保护文件锁表
//!
//! # 文件类型
//!
//! VFS 支持多种文件类型，所有类型都实现统一的 [`File`] trait：
//!
//! - [`RegFile`]: 普通文件 - 基于 Inode，支持 seek
//! - [`PipeFile`]: 管道文件 - 环形缓冲区，流式设备
//! - [`StdinFile`]/[`StdoutFile`]/[`StderrFile`]: 标准 I/O 文件
//! - `CharDevFile`: 字符设备文件（串口、终端等）
//! - `BlkDevFile`: 块设备文件（磁盘等）
//!
//! # 使用示例
//!
//! ## 基本文件操作
//!
//! ```rust
//! use vfs::{vfs_lookup, RegFile, OpenFlags};
//! use alloc::sync::Arc;
//!
//! // 1. 查找文件
//! let dentry = vfs_lookup("/etc/passwd")?;
//!
//! // 2. 创建 File 对象
//! let file = Arc::new(RegFile::new(dentry, OpenFlags::O_RDONLY));
//!
//! // 3. 读取数据
//! let mut buf = [0u8; 1024];
//! let n = file.read(&mut buf)?;
//! ```
//!
//! ## 使用文件描述符
//!
//! ```rust
//! // 分配文件描述符
//! let fd_table = current_task().lock().fd_table.clone();
//! let fd = fd_table.alloc(file)?;
//!
//! // 通过 FD 访问文件
//! let file = fd_table.get(fd)?;
//! file.read(&mut buf)?;
//!
//! // 关闭文件
//! fd_table.close(fd)?;
//! ```
//!
//! ## 挂载文件系统
//!
//! ```rust
//! use vfs::{MOUNT_TABLE, MountFlags};
//!
//! // 创建文件系统
//! let tmpfs = Arc::new(TmpFs::new());
//!
//! // 挂载到 /tmp
//! MOUNT_TABLE.mount(tmpfs, "/tmp", MountFlags::empty(), None)?;
//!
//! // 访问挂载点下的文件
//! let dentry = vfs_lookup("/tmp/test.txt")?;
//!
//! // 卸载
//! MOUNT_TABLE.umount("/tmp")?;
//! ```
//!
//! ## 创建管道
//!
//! ```rust
//! use vfs::PipeFile;
//!
//! let (read_file, write_file) = PipeFile::create_pipe()?;
//!
//! // 分配文件描述符
//! let read_fd = fd_table.alloc(read_file)?;
//! let write_fd = fd_table.alloc(write_file)?;
//!
//! // 父子进程通过管道通信
//! ```

pub mod adapter;
pub mod dentry;
pub mod dev;
pub mod devno;
pub mod error;
pub mod fd_table;
pub mod file;
pub mod file_lock;
pub mod file_system;
pub mod impls;
pub mod inode;
pub mod mount;
pub mod path;

pub use adapter::inode_type_to_d_type;
pub use dentry::{DENTRY_CACHE, Dentry, DentryCache};
pub use dev::{major, makedev, minor};
pub use devno::{get_blkdev_index, get_chrdev_driver};
pub use error::FsError;
pub use fd_table::FDTable;
pub use file::File;
pub use file_lock::file_lock_manager;
pub use file_system::{FileSystem, StatFs};
pub use impls::{PipeFile, RegFile, StderrFile, StdinFile, StdoutFile, create_stdio_files};
pub use inode::{DirEntry, FileMode, Inode, InodeMetadata, InodeType};
pub use mount::{MOUNT_TABLE, MountFlags, MountPoint, MountTable, get_root_dentry};
pub use path::{
    normalize_path, parse_path, split_path, vfs_lookup, vfs_lookup_from, vfs_lookup_no_follow,
    vfs_lookup_no_follow_from,
};

// Re-export UAPI types used by VFS
pub use crate::uapi::fcntl::{FdFlags, OpenFlags, SeekWhence};
pub use crate::uapi::fs::{LinuxDirent64, Stat, Statx};
pub use crate::uapi::time::TimeSpec;

use alloc::{vec, vec::Vec};

/// 从指定路径加载 ELF 文件内容
///
/// 参数：
///     - path: 文件路径（绝对路径或相对于当前工作目录的相对路径）
///
/// 返回：`Ok(Vec<u8>)` 文件内容字节数组；`Err(FsError::NotFound)` 文件不存在；`Err(FsError::IsDirectory)` 路径指向目录
pub fn vfs_load_elf(path: &str) -> Result<Vec<u8>, FsError> {
    let dentry = vfs_lookup(path)?;
    let inode = &dentry.inode;
    let metadata = inode.metadata()?;

    // 确保是普通文件
    if metadata.inode_type != InodeType::File {
        return Err(FsError::IsDirectory);
    }

    let mut buf = vec![0u8; metadata.size];
    inode.read_at(0, &mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests;
