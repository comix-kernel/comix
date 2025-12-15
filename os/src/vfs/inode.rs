//! Inode 抽象层 - VFS 存储层接口
//!
//! 该模块定义了文件系统的底层存储接口，提供无状态的文件和目录访问能力。
//!
//! # 组件
//!
//! - [`Inode`] trait：文件/目录的核心操作接口
//! - [`InodeMetadata`]：文件元数据（大小、权限、时间戳等）
//! - [`InodeType`]：文件类型枚举（普通文件、目录、符号链接等）
//! - [`FileMode`]：POSIX 兼容的权限位
//! - [`DirEntry`]：轻量级目录项（用于 readdir）
//!
//! # 与 File trait 的关系
//!
//! | 方面 | File (会话层) | Inode (存储层) |
//! |------|---------------|----------------|
//! | 状态 | 有状态 (offset、flags) | 无状态 |
//! | 方法 | `read(buf)` | `read_at(offset, buf)` |
//! | 实例 | 每次 open 创建新实例 | 多个 File 可共享同一 Inode |
//! | 职责 | 维护会话状态 | 提供存储访问 |
//!
//! ## 为什么 Inode 是无状态的?
//!
//! 无状态设计使得：
//! - 同一文件可被多次打开，各自维护独立的 offset
//! - 硬链接可以共享同一个 Inode
//! - 多线程读写时无需锁定 Inode（offset 由 File 管理）
//!
//! ```text
//! File { offset: 0 }  ──┐
//!                       ├──> Inode { data: [...] }
//! File { offset: 100 }──┘
//! ```
//!
//! # 设计要点
//!
//! ## 随机访问
//!
//! 所有读写方法都携带 `offset` 参数，支持真正的随机访问：
//!
//! ```rust
//! // 可以乱序访问，不影响彼此
//! inode.read_at(100, &mut buf1)?;  // 读取位置 100
//! inode.read_at(0, &mut buf2)?;    // 读取位置 0
//! ```
//!
//! ## 幂等性
//!
//! 读取操作应该是幂等的（多次调用返回相同结果）：
//!
//! ```rust
//! let n1 = inode.read_at(0, &mut buf)?;
//! let n2 = inode.read_at(0, &mut buf)?;
//! assert_eq!(n1, n2);  // 相同的读取结果
//! ```
//!
//! ## 目录操作
//!
//! Inode 不仅支持文件读写，还支持目录操作：
//!
//! - `lookup(name)` - 查找子项
//! - `create(name)` - 创建文件
//! - `mkdir(name)` - 创建目录
//! - `unlink(name)` - 删除文件
//! - `readdir()` - 列出目录内容
//!
//! # 使用示例
//!
//! ```rust
//! use vfs::{Inode, FileMode};
//!
//! // 1. 读写文件
//! let mut buf = [0u8; 512];
//! let n = inode.read_at(0, &mut buf)?;
//! inode.write_at(512, b"hello")?;
//!
//! // 2. 目录操作
//! let child_inode = parent_inode.lookup("file.txt")?;
//! parent_inode.create("new.txt", FileMode::S_IFREG | FileMode::S_IRUSR)?;
//!
//! // 3. 获取元数据
//! let metadata = inode.metadata()?;
//! println!("大小: {}, 类型: {:?}", metadata.size, metadata.inode_type);
//! ```

use core::any::Any;

use crate::uapi::time::TimeSpec;
use crate::vfs::{Dentry, FsError};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;

/// 文件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InodeType {
    File,        // 普通文件
    Directory,   // 目录
    Symlink,     // 符号链接
    CharDevice,  // 字符设备
    BlockDevice, // 块设备
    Fifo,        // 命名管道
    Socket,      // 套接字
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    /// 文件权限和类型（与 POSIX 兼容）
    pub struct FileMode: u32 {
        // 文件类型掩码
        const S_IFMT   = 0o170000;  // 文件类型掩码
        const S_IFREG  = 0o100000;  // 普通文件
        const S_IFDIR  = 0o040000;  // 目录
        const S_IFLNK  = 0o120000;  // 符号链接
        const S_IFCHR  = 0o020000;  // 字符设备
        const S_IFBLK  = 0o060000;  // 块设备
        const S_IFIFO  = 0o010000;  // FIFO
        const S_IFSOCK = 0o140000;  // Socket

        // 用户权限
        const S_IRUSR  = 0o400;     // 用户读
        const S_IWUSR  = 0o200;     // 用户写
        const S_IXUSR  = 0o100;     // 用户执行

        // 组权限
        const S_IRGRP  = 0o040;     // 组读
        const S_IWGRP  = 0o020;     // 组写
        const S_IXGRP  = 0o010;     // 组执行

        // 其他用户权限
        const S_IROTH  = 0o004;     // 其他读
        const S_IWOTH  = 0o002;     // 其他写
        const S_IXOTH  = 0o001;     // 其他执行

        // 特殊位
        const S_ISUID  = 0o4000;    // Set UID
        const S_ISGID  = 0o2000;    // Set GID
        const S_ISVTX  = 0o1000;    // Sticky bit
    }
}

impl FileMode {
    /// 检查是否有读权限（暂时只检查用户权限）
    pub fn can_read(&self) -> bool {
        self.contains(FileMode::S_IRUSR)
    }

    /// 检查是否有写权限
    pub fn can_write(&self) -> bool {
        self.contains(FileMode::S_IWUSR)
    }

    /// 检查是否有执行权限
    pub fn can_execute(&self) -> bool {
        self.contains(FileMode::S_IXUSR)
    }
}

/// 轻量级目录项（readdir 返回）
///
/// 用于数据传输，无引用关系，读取后即可丢弃
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,          // 文件名
    pub inode_no: usize,       // Inode 编号
    pub inode_type: InodeType, // 文件类型
}

/// 文件元数据
#[derive(Debug, Clone)]
pub struct InodeMetadata {
    pub inode_no: usize,       // Inode 编号
    pub inode_type: InodeType, // 文件类型
    pub mode: FileMode,        // 权限位
    pub uid: u32,              // 用户 ID
    pub gid: u32,              // 组 ID
    pub size: usize,           // 文件大小（字节）
    pub atime: TimeSpec,       // 访问时间
    pub mtime: TimeSpec,       // 修改时间
    pub ctime: TimeSpec,       // 状态改变时间
    pub nlinks: usize,         // 硬链接数
    pub blocks: usize,         // 占用的块数（512B 为单位）
    pub rdev: u64,             // 设备号（仅对 CharDevice 和 BlockDevice 有效）
}

/// 文件系统底层存储接口
///
/// Inode 代表文件系统中的一个文件或目录，提供无状态的随机访问。
///
/// # 设计要点
///
/// - 所有读写方法必须携带 `offset` 参数（体现随机访问能力）
/// - 不维护会话状态（offset 由上层 File 维护）
/// - 支持目录操作（lookup、create、mkdir、unlink）
pub trait Inode: Send + Sync + Any {
    /// 获取文件元数据
    fn metadata(&self) -> Result<InodeMetadata, FsError>;

    /// 从指定偏移量读取数据
    ///
    /// 多次调用相同参数应返回相同结果（无副作用）。
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError>;

    /// 向指定偏移量写入数据
    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError>;

    /// 在目录中查找子项
    ///
    /// 返回子项的 Inode。仅对目录有效。
    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, FsError>;

    /// 在目录中创建文件
    fn create(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError>;

    /// 在目录中创建子目录
    fn mkdir(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError>;

    /// 创建符号链接
    fn symlink(&self, name: &str, target: &str) -> Result<Arc<dyn Inode>, FsError>;

    /// 创建硬链接
    fn link(&self, name: &str, target: &Arc<dyn Inode>) -> Result<(), FsError>;

    /// 删除普通文件/链接
    fn unlink(&self, name: &str) -> Result<(), FsError>;

    /// 删除目录
    fn rmdir(&self, name: &str) -> Result<(), FsError>;

    /// 重命名/移动 (原子操作)
    fn rename(
        &self,
        old_name: &str,
        new_parent: Arc<dyn Inode>,
        new_name: &str,
    ) -> Result<(), FsError>;

    /// 列出目录内容
    fn readdir(&self) -> Result<Vec<DirEntry>, FsError>;

    /// 截断文件到指定大小
    fn truncate(&self, size: usize) -> Result<(), FsError>;

    /// 同步文件数据到存储设备
    fn sync(&self) -> Result<(), FsError>;

    /// 设置 Dentry（可选方法）
    fn set_dentry(&self, _dentry: Weak<Dentry>) {}

    /// 获取 Dentry（可选方法）
    fn get_dentry(&self) -> Option<Arc<Dentry>> {
        None
    }

    /// 向下转型为 &dyn Any，用于支持 downcast
    fn as_any(&self) -> &dyn Any;

    /// 设置文件时间戳
    fn set_times(&self, atime: Option<TimeSpec>, mtime: Option<TimeSpec>) -> Result<(), FsError>;

    /// 读取符号链接的目标路径
    fn readlink(&self) -> Result<String, FsError>;

    /// 创建设备文件节点
    fn mknod(&self, name: &str, mode: FileMode, dev: u64) -> Result<Arc<dyn Inode>, FsError>;

    /// 修改文件所有者和组
    ///
    /// # 参数
    /// * `uid` - 新的用户 ID（`u32::MAX` 表示不改变）
    /// * `gid` - 新的组 ID（`u32::MAX` 表示不改变）
    ///
    /// # 返回值
    /// * `Ok(())` - 成功
    /// * `Err(FsError)` - 失败
    ///
    /// # 在单 root 用户系统中的行为
    /// 此方法会更新 inode 的 uid/gid 字段，但不进行权限检查。
    /// 所有调用都会成功（除非文件系统错误）。
    fn chown(&self, _uid: u32, _gid: u32) -> Result<(), FsError>;

    /// 修改文件权限模式
    ///
    /// # 参数
    /// * `mode` - 新的权限模式（只修改权限位，不修改文件类型位）
    ///
    /// # 返回值
    /// * `Ok(())` - 成功
    /// * `Err(FsError)` - 失败
    ///
    /// # 在单 root 用户系统中的行为
    /// 此方法会更新 inode 的 mode 字段，但不进行权限检查。
    /// 所有调用都会成功（除非文件系统错误）。
    fn chmod(&self, _mode: FileMode) -> Result<(), FsError>;
}

/// 为 `Arc<dyn Inode>` 提供向下转型辅助方法
impl dyn Inode {
    /// 尝试向下转型为具体的 Inode 类型
    pub fn downcast_arc<T: Inode>(self: Arc<Self>) -> Result<Arc<T>, Arc<Self>> {
        if (*self).as_any().is::<T>() {
            // SAFETY: 已经通过 is::<T>() 检查了类型
            unsafe {
                let ptr = Arc::into_raw(self);
                Ok(Arc::from_raw(ptr as *const T))
            }
        } else {
            Err(self)
        }
    }

    /// 尝试获取具体类型的引用
    pub fn downcast_ref<T: Inode>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}
