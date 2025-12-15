//! 挂载表管理
//!
//! 该模块实现了 VFS 的挂载功能，支持在不同路径挂载多个文件系统，类似 Linux 的挂载机制。
//!
//! # 核心组件
//!
//! - [`MountTable`] - 全局挂载表，管理所有挂载点
//! - [`MountPoint`] - 单个挂载点信息
//! - [`MountFlags`] - 挂载标志（只读、禁止执行等）
//! - [`MOUNT_TABLE`] - 全局挂载表单例
//! - [`get_root_dentry`] - 获取根文件系统的 Dentry
//!
//! # 核心概念
//!
//! ## 挂载的本质
//!
//! 挂载是将一个文件系统的根目录"覆盖"到另一个文件系统的某个目录上：
//!
//! ```text
//! 挂载前:
//! /
//! ├── mnt/          (tmpfs 目录)
//! │   └── old_file
//! └── etc/
//!
//! 挂载 fat32 到 /mnt 后:
//! /
//! ├── mnt/ ─┬─> tmpfs 的 /mnt/ (被覆盖)
//! │         └─> fat32 的 / (新可见)
//! │             ├── file1.txt
//! │             └── file2.txt
//! └── etc/
//! ```
//!
//! ## 挂载点栈
//!
//! 同一路径可以多次挂载，形成栈结构，最后挂载的文件系统覆盖前面的：
//!
//! ```text
//! /mnt → [tmpfs1, tmpfs2, fat32]
//!                          ↑
//!                      当前可见
//!
//! umount /mnt → [tmpfs1, tmpfs2]
//!                        ↑
//!                    tmpfs2 变为可见
//! ```
//!
//! ## 最长前缀匹配
//!
//! 访问路径时，选择最长匹配的挂载点：
//!
//! ```text
//! 挂载情况:
//! /          → tmpfs
//! /mnt       → fat32
//! /mnt/data  → ext4
//!
//! 访问 "/mnt/data/file" → 使用 ext4 (最长匹配)
//! 访问 "/mnt/other"     → 使用 fat32
//! 访问 "/etc/passwd"    → 使用 tmpfs (根目录)
//! ```
//!
//! # 挂载标志
//!
//! ## MountFlags
//!
//! ```rust
//! bitflags! {
//!     pub struct MountFlags: u32 {
//!         const READ_ONLY  = 1 << 0;  // 只读挂载
//!         const NO_EXEC    = 1 << 1;  // 禁止执行
//!         const NO_SUID    = 1 << 2;  // 忽略 SUID/SGID
//!         const SYNC       = 1 << 3;  // 同步写入
//!         const NO_DEV     = 1 << 4;  // 禁止设备文件
//!     }
//! }
//! ```
//!
//! ## 标志用途
//!
//! - **READ_ONLY**: 防止修改文件系统（如挂载只读光盘）
//! - **NO_EXEC**: 安全策略，防止执行恶意程序
//! - **NO_SUID**: 防止特权提升攻击
//! - **SYNC**: 确保数据持久化（牺牲性能）
//! - **NO_DEV**: 防止通过设备文件访问硬件
//!
//! # MountPoint 结构
//!
//! ```rust
//! pub struct MountPoint {
//!     pub fs: Arc<dyn FileSystem>,  // 挂载的文件系统
//!     pub root: Arc<Dentry>,        // 文件系统的根 Dentry
//!     pub flags: MountFlags,        // 挂载标志
//!     pub device: Option<String>,   // 设备路径（如 "/dev/sda1"）
//!     pub mount_path: String,       // 挂载路径
//! }
//! ```
//!
//! # 挂载表实现
//!
//! ## 数据结构
//!
//! ```text
//! MountTable
//! ┌─────────────┬────────────────────────┐
//! │ "/"         │ [tmpfs_root]           │
//! │ "/mnt"      │ [fat32_mp]             │
//! │ "/mnt/data" │ [ext4_mp1, ext4_mp2]   │
//! └─────────────┴────────────────────────┘
//!                              ↑
//!                          栈顶=当前可见
//! ```
//!
//! ## 线程安全
//!
//! 挂载表使用 `SpinLock` 保护，支持并发访问：
//!
//! ```rust
//! struct MountTable {
//!     mounts: SpinLock<BTreeMap<String, Vec<Arc<MountPoint>>>>,
//! }
//! ```
//!
//! # 使用示例
//!
//! ## 挂载文件系统
//!
//! ```rust
//! use vfs::{MOUNT_TABLE, MountFlags};
//! use alloc::sync::Arc;
//!
//! // 1. 创建文件系统
//! let tmpfs = Arc::new(TmpFs::new());
//!
//! // 2. 挂载到 /tmp
//! MOUNT_TABLE.mount(
//!     tmpfs,
//!     "/tmp",
//!     MountFlags::empty(),
//!     None
//! )?;
//!
//! // 3. 只读挂载
//! MOUNT_TABLE.mount(
//!     fat32_fs,
//!     "/mnt/usb",
//!     MountFlags::READ_ONLY | MountFlags::NO_EXEC,
//!     Some(String::from("/dev/sda1"))
//! )?;
//! ```
//!
//! ## 卸载文件系统
//!
//! ```rust
//! // 卸载 /tmp
//! MOUNT_TABLE.umount("/tmp")?;
//!
//! // 注意：不能卸载根文件系统
//! MOUNT_TABLE.umount("/")?;  // 返回 Err(NotSupported)
//! ```
//!
//! ## 查找挂载点
//!
//! ```rust
//! // 查找路径对应的挂载点
//! if let Some(mp) = MOUNT_TABLE.find_mount("/mnt/data/file") {
//!     println!("文件系统类型: {}", mp.fs.fs_type());
//!     println!("挂载路径: {}", mp.mount_path);
//! }
//! ```
//!
//! ## 列出所有挂载点
//!
//! ```rust
//! // 调试用：列出所有挂载
//! let mounts = MOUNT_TABLE.list_mounts();
//! for (path, fstype) in mounts {
//!     println!("{} on {} type {}", path, path, fstype);
//! }
//! // 输出:
//! // / on / type tmpfs
//! // /mnt on /mnt type fat32
//! // /mnt/data on /mnt/data type ext4
//! ```
//!
//! ## 系统初始化挂载根文件系统
//!
//! ```rust
//! pub fn init_rootfs() -> Result<(), FsError> {
//!     // 创建 tmpfs 作为根文件系统
//!     let tmpfs = Arc::new(TmpFs::new());
//!
//!     // 挂载到 /
//!     MOUNT_TABLE.mount(tmpfs, "/", MountFlags::empty(), None)?;
//!
//!     // 创建基本目录结构
//!     let root = get_root_dentry()?;
//!     root.inode.mkdir("dev", FileMode::S_IFDIR | FileMode::S_IRWXU)?;
//!     root.inode.mkdir("etc", FileMode::S_IFDIR | FileMode::S_IRWXU)?;
//!     root.inode.mkdir("tmp", FileMode::S_IFDIR | FileMode::S_IRWXU)?;
//!     root.inode.mkdir("mnt", FileMode::S_IFDIR | FileMode::S_IRWXU)?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # 挂载与 Dentry 缓存的交互
//!
//! 挂载和卸载时会自动更新 Dentry 缓存：
//!
//! ```rust
//! // 挂载时：如果挂载点 Dentry 在缓存中，更新其挂载信息
//! if let Some(dentry) = DENTRY_CACHE.lookup(&mount_path) {
//!     dentry.set_mount(&mount_point.root);
//! }
//!
//! // 卸载时：恢复为下层挂载或清除挂载标记
//! if let Some(dentry) = DENTRY_CACHE.lookup(&mount_path) {
//!     if let Some(underlying) = underlying_mount {
//!         dentry.set_mount(&underlying.root);
//!     } else {
//!         dentry.clear_mount();
//!     }
//! }
//! ```

use crate::sync::SpinLock;
use crate::vfs::{Dentry, FileMode, FileSystem, FsError};
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

/// 挂载标志
bitflags::bitflags! {
    pub struct MountFlags: u32 {
        /// 只读挂载
        const READ_ONLY  = 1 << 0;

        /// 禁止执行
        const NO_EXEC    = 1 << 1;

        /// 忽略 SUID/SGID 位
        const NO_SUID    = 1 << 2;

        /// 同步写入
        const SYNC       = 1 << 3;

        /// 禁止设备文件
        const NO_DEV     = 1 << 4;
    }
}

/// 挂载点信息
pub struct MountPoint {
    /// 挂载的文件系统
    pub fs: Arc<dyn FileSystem>,

    /// 挂载点的根 dentry
    pub root: Arc<Dentry>,

    /// 挂载标志
    pub flags: MountFlags,

    /// 设备路径（如果有）
    pub device: Option<String>,

    /// 挂载路径
    pub mount_path: String,
}

impl MountPoint {
    /// 创建新的挂载点
    pub fn new(
        fs: Arc<dyn FileSystem>,
        mount_path: String,
        flags: MountFlags,
        device: Option<String>,
    ) -> Arc<Self> {
        let root_inode = fs.root_inode();
        let root = Dentry::new(String::from("/"), root_inode);

        Arc::new(Self {
            fs,
            root,
            flags,
            device,
            mount_path,
        })
    }
}

/// 全局挂载表
pub struct MountTable {
    /// 挂载路径 -> 挂载点栈（最后一个是当前可见的）
    mounts: SpinLock<BTreeMap<String, Vec<Arc<MountPoint>>>>,
}

impl MountTable {
    /// 创建新的挂载表
    pub const fn new() -> Self {
        Self {
            mounts: SpinLock::new(BTreeMap::new()),
        }
    }

    /// 挂载文件系统
    pub fn mount(
        &self,
        fs: Arc<dyn FileSystem>,
        path: &str,
        flags: MountFlags,
        device: Option<String>,
    ) -> Result<(), FsError> {
        use crate::vfs::normalize_path;

        let normalized_path = normalize_path(path);

        // 创建挂载点
        let mount_point = MountPoint::new(fs, normalized_path.clone(), flags, device);

        // 添加到挂载栈
        let mut mounts = self.mounts.lock();
        mounts
            .entry(normalized_path.clone())
            .or_insert_with(Vec::new)
            .push(mount_point.clone());

        // 如果挂载点的 dentry 已经存在于缓存中，更新其挂载信息
        if let Some(dentry) = crate::vfs::DENTRY_CACHE.lookup(&normalized_path) {
            dentry.set_mount(&mount_point.root);
        }

        Ok(())
    }

    /// 卸载文件系统
    pub fn umount(&self, path: &str) -> Result<(), FsError> {
        use crate::vfs::normalize_path;

        let normalized_path = normalize_path(path);

        // 不允许卸载根文件系统
        if normalized_path == "/" {
            return Err(FsError::NotSupported);
        }

        let mut mounts = self.mounts.lock();
        let stack = mounts.get_mut(&normalized_path).ok_or(FsError::NotFound)?;

        // 弹出栈顶的挂载点
        let mount_point = stack.pop().ok_or(FsError::NotFound)?;

        // 如果栈为空，移除整个条目
        if stack.is_empty() {
            mounts.remove(&normalized_path);
        }

        // 释放锁，避免在同步/卸载时持有锁
        drop(mounts);

        // 同步文件系统
        mount_point.fs.sync()?;

        // 执行卸载清理
        mount_point.fs.umount()?;

        // 更新 dentry 缓存
        if let Some(dentry) = crate::vfs::DENTRY_CACHE.lookup(&normalized_path) {
            // 如果还有下层挂载，更新为下层挂载点
            let mounts = self.mounts.lock();
            if let Some(stack) = mounts.get(&normalized_path) {
                if let Some(underlying_mount) = stack.last() {
                    dentry.set_mount(&underlying_mount.root);
                } else {
                    dentry.clear_mount();
                }
            } else {
                dentry.clear_mount();
            }
        }

        Ok(())
    }

    /// 查找给定路径的挂载点
    ///
    /// 返回最长匹配的挂载点（栈顶）
    pub fn find_mount(&self, path: &str) -> Option<Arc<MountPoint>> {
        use crate::vfs::normalize_path;

        let normalized_path = normalize_path(path);
        let mounts = self.mounts.lock();

        // 查找最长匹配的挂载点
        let mut best_match = None;
        let mut best_len = 0;

        for (mount_path, stack) in mounts.iter() {
            if normalized_path.starts_with(mount_path) && mount_path.len() > best_len {
                // 返回栈顶的挂载点（当前可见的）
                if let Some(mp) = stack.last() {
                    best_match = Some(mp.clone());
                    best_len = mount_path.len();
                }
            }
        }

        best_match
    }

    /// 获取根挂载点
    pub fn root_mount(&self) -> Option<Arc<MountPoint>> {
        self.mounts
            .lock()
            .get("/")
            .and_then(|stack| stack.last())
            .cloned()
    }

    /// 列出所有挂载点（用于调试）
    pub fn list_mounts(&self) -> Vec<(String, String)> {
        let mounts = self.mounts.lock();
        mounts
            .iter()
            .flat_map(|(path, stack)| {
                stack
                    .iter()
                    .map(|mp| (path.clone(), String::from(mp.fs.fs_type())))
            })
            .collect()
    }

    pub fn list_all(&self) -> BTreeMap<String, Arc<MountPoint>> {
        let mounts = self.mounts.lock();
        mounts
            .iter() // 获取引用，不消耗原 Map
            .filter_map(|(key, stack)| {
                // 1. stack.last(): 获取栈顶元素的引用（如果不为空）
                // 2. map(...): 如果栈顶存在，执行闭包
                stack.last().map(|mount_point| {
                    (
                        key.clone(),         // 必须克隆 String，因为新 Map 需要拥有 Key 的所有权
                        mount_point.clone(), // 克隆 Arc，这非常廉价（只增加引用计数），不涉及深拷贝
                    )
                })
            })
            .collect()
    }
}

// 全局挂载表
lazy_static::lazy_static! {
    pub static ref MOUNT_TABLE: MountTable = MountTable::new();
}

/// 获取根 dentry
pub fn get_root_dentry() -> Result<Arc<Dentry>, FsError> {
    MOUNT_TABLE
        .root_mount()
        .map(|mp| mp.root.clone())
        .ok_or(FsError::NotSupported)
}
