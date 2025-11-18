//! 虚拟文件系统（VFS）层
//!
//! 提供统一的文件系统抽象接口，支持多种文件类型和文件系统。
//!
//! # 架构
//!
//! VFS 采用分层设计：
//!
//! - **会话层** ([`File`] trait): 维护打开文件的会话状态（offset、flags），以 `Arc<dyn File>` 形式存储于进程文件描述符表中
//! - **存储层** ([`Inode`] trait): 提供无状态的随机访问接口（`read_at`、`write_at`）
//! - **路径层** ([`Dentry`]、[`path`] 模块): 管理目录树结构和路径解析
//! - **挂载层** ([`mount`] 模块): 支持多文件系统挂载
//!
//! # 文件类型
//!
//! - [`DiskFile`]: 基于 Inode 的磁盘文件（支持 lseek）
//! - [`PipeFile`]: 管道文件（流式设备，不支持 lseek）
//! - [`StdinFile`]/[`StdoutFile`]/[`StderrFile`]: 标准 I/O 文件（字符设备）
//!
//! # 示例
//!
//! ```rust
//! // 打开文件
//! let dentry = vfs_lookup("/etc/passwd")?;
//! let file = DiskFile::new(dentry, OpenFlags::O_RDONLY);
//! let file: Arc<dyn File> = Arc::new(file);
//!
//! // 安装到文件描述符表
//! fd_table.install_at(3, file)?;
//! ```

pub mod dentry;
pub mod error;
pub mod fd_table;
pub mod file;
pub mod file_system;
pub mod impls;
pub mod inode;
pub mod mount;
pub mod path;

pub use dentry::{DENTRY_CACHE, Dentry, DentryCache};
pub use error::FsError;
pub use fd_table::FDTable;
pub use file::{File, OpenFlags, SeekWhence};
pub use file_system::{FileSystem, StatFs};
pub use impls::{DiskFile, PipeFile, StderrFile, StdinFile, StdoutFile, create_stdio_files};
pub use inode::{DirEntry, FileMode, Inode, InodeMetadata, InodeType, TimeSpec};
pub use mount::{MOUNT_TABLE, MountFlags, MountPoint, MountTable, get_root_dentry};
pub use path::{normalize_path, parse_path, split_path, vfs_lookup};

use alloc::{vec, vec::Vec};

/// 从指定路径加载 ELF 文件内容
///
/// 参数：
///     - path: 文件路径（绝对路径或相对于当前工作目录的相对路径）
///
/// 返回：Ok(Vec<u8>) 文件内容字节数组；Err(FsError::NotFound) 文件不存在；Err(FsError::IsDirectory) 路径指向目录
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
