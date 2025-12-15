//! Ext4 - Linux Ext4 文件系统支持
//!
//! 该模块提供了与 **Linux Ext4 文件系统兼容** 的读写支持，基于 `ext4_rs` crate 实现。
//!
//! # 组件
//!
//! - [`Ext4FileSystem`] - 文件系统结构，实现 `FileSystem` trait
//! - [`Ext4Inode`] - Inode 包装，将 `ext4_rs` 操作映射到 VFS
//! - [`BlockDeviceAdapter`] - 块设备适配器，桥接 VirtIO 和 ext4_rs
//!
//! # 设计概览
//!
//! ## 适配层架构
//!
//! ```text
//! VFS (Inode trait)
//!       ↓
//! Ext4Inode (包装层)
//!       ↓
//! ext4_rs::Ext4 (第三方库)
//!       ↓
//! BlockDeviceAdapter
//!       ↓
//! BlockDriver (VirtIO Block)
//! ```
//!
//! ## 支持的操作
//!
//! - **文件操作**：read、write、truncate、sync
//! - **目录操作**：lookup、create、mkdir、readdir、rmdir
//! - **链接操作**：symlink、link、unlink、readlink
//! - **元数据**：chmod、chown、set_times
//! - **重命名**：rename（支持跨目录移动）
//!
//! # 使用示例
//!
//! ```rust
//! use crate::fs::init_ext4_from_block_device;
//!
//! // 从第一个块设备挂载 ext4 为根文件系统
//! init_ext4_from_block_device()?;
//!
//! // 读写文件
//! let content = vfs_load_file("/bin/hello")?;
//! ```
//!
//! # 配置要求
//!
//! - **块大小**：必须为 4096 字节（与 `mkfs.ext4 -b 4096` 匹配）
//! - **块设备**：需要支持 VirtIO Block 或兼容驱动
//!
//! # 限制
//!
//! - `mknod` 未实现（设备文件创建）
//! - 非日志模式，崩溃可能导致不一致
pub mod adpaters;
pub mod inode;

pub use adpaters::BlockDeviceAdapter;
pub use inode::Ext4Inode;

use crate::device::block::BlockDriver;
use crate::pr_info;
use crate::sync::Mutex;
use crate::vfs::{FileSystem, FsError, Inode, StatFs};
use alloc::sync::Arc;

/// Ext4 文件系统
pub struct Ext4FileSystem {
    /// 底层块设备驱动
    device: Arc<dyn BlockDriver>,

    /// 块大小
    block_size: usize,

    /// 总块数
    total_blocks: usize,

    /// 设备 ID
    device_id: usize,

    /// ext4_rs 文件系统对象
    ext4: Arc<Mutex<ext4_rs::Ext4>>,

    /// 根 inode
    root: Arc<dyn Inode>,
}

impl Ext4FileSystem {
    /// 打开 Ext4 文件系统
    ///
    /// # 参数
    /// - `device`: 块设备驱动
    /// - `block_size`: 块大小
    /// - `total_blocks`: 总块数
    /// - `device_id`: 设备 ID
    ///
    /// # 返回
    /// Ext4 文件系统实例
    pub fn open(
        device: Arc<dyn BlockDriver>,
        block_size: usize,
        total_blocks: usize,
        device_id: usize,
    ) -> Result<Arc<Self>, FsError> {
        pr_info!("[Ext4] Opening Ext4 filesystem on block device");
        pr_info!(
            "[Ext4] Device block size: {}, total blocks: {}",
            block_size,
            total_blocks
        );

        // 创建适配器
        let adapter = Arc::new(BlockDeviceAdapter::new(device.clone(), block_size));

        // 使用 ext4_rs 打开文件系统
        // 注意：ext4_rs::Ext4::open 直接返回 Ext4，不返回 Result
        pr_info!("[Ext4] Calling ext4_rs::Ext4::open...");
        let ext4 = ext4_rs::Ext4::open(adapter);
        pr_info!("[Ext4] ext4_rs returned successfully");

        let ext4 = Arc::new(Mutex::new(ext4));

        // 创建根 inode (inode 号 2 是 Ext4 的根目录)
        let root = Arc::new(Ext4Inode::new(ext4.clone(), 2));

        let fs = Arc::new(Ext4FileSystem {
            device,
            block_size,
            total_blocks,
            device_id,
            ext4,
            root,
        });

        pr_info!("[Ext4] Filesystem opened successfully");
        Ok(fs)
    }
}

impl FileSystem for Ext4FileSystem {
    fn fs_type(&self) -> &'static str {
        "ext4"
    }

    fn root_inode(&self) -> Arc<dyn Inode> {
        self.root.clone()
    }

    fn sync(&self) -> Result<(), FsError> {
        // 调用底层块设备的 flush 方法，将缓存刷新到磁盘
        if self.device.flush() {
            Ok(())
        } else {
            Err(FsError::IoError)
        }
    }

    fn statfs(&self) -> Result<StatFs, FsError> {
        let ext4 = self.ext4.lock();
        let sb = &ext4.super_block;

        Ok(StatFs {
            block_size: self.block_size,
            total_blocks: self.total_blocks,
            free_blocks: sb.free_blocks_count() as usize,
            available_blocks: sb.free_blocks_count() as usize,
            total_inodes: sb.inodes_count as usize,
            free_inodes: sb.free_inodes_count() as usize,
            fsid: self.device_id as u64,
            max_filename_len: 255, // EXT4_NAME_LEN
        })
    }
}
