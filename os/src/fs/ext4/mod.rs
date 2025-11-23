//! Ext4 文件系统实现
//!
//! 基于 ext4_rs crate，提供 VFS 接口

pub mod adpaters;
pub mod inode;

pub use adpaters::BlockDeviceAdapter;
pub use inode::Ext4Inode;

use crate::device::block::block_device::BlockDevice as VfsBlockDevice;
use crate::pr_info;
use crate::sync::SpinLock;
use crate::vfs::{FileSystem, FsError, Inode, StatFs};
use alloc::sync::Arc;

/// Ext4 文件系统
pub struct Ext4FileSystem {
    /// 底层块设备
    device: Arc<dyn VfsBlockDevice>,

    /// ext4_rs 文件系统对象
    ext4: Arc<SpinLock<ext4_rs::Ext4>>,

    /// 根 inode
    root: Arc<dyn Inode>,
}

impl Ext4FileSystem {
    /// 打开 Ext4 文件系统
    ///
    /// # 参数
    /// - `device`: 块设备 (RamDisk 或 VirtIOBlock)
    ///
    /// # 返回
    /// Ext4 文件系统实例
    pub fn open(device: Arc<dyn VfsBlockDevice>) -> Result<Arc<Self>, FsError> {
        pr_info!("[Ext4] Opening Ext4 filesystem on block device");
        pr_info!(
            "[Ext4] Device block size: {}, total blocks: {}",
            device.block_size(),
            device.total_blocks()
        );

        // 创建适配器
        let adapter = Arc::new(BlockDeviceAdapter::new(device.clone()));

        // 使用 ext4_rs 打开文件系统
        // 注意：ext4_rs::Ext4::open 直接返回 Ext4，不返回 Result
        pr_info!("[Ext4] Calling ext4_rs::Ext4::open...");
        let ext4 = ext4_rs::Ext4::open(adapter);
        pr_info!("[Ext4] ext4_rs returned successfully");

        let ext4 = Arc::new(SpinLock::new(ext4));

        // 创建根 inode (inode 号 2 是 Ext4 的根目录)
        let root = Arc::new(Ext4Inode::new(ext4.clone(), 2)); // ← 不再传 path

        let fs = Arc::new(Ext4FileSystem { device, ext4, root });

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
        // TODO: ext4_rs 可能需要实现 sync 方法
        self.device.flush().map_err(|_| FsError::IoError)
    }

    fn statfs(&self) -> Result<StatFs, FsError> {
        let ext4 = self.ext4.lock();
        let sb = &ext4.super_block;

        Ok(StatFs {
            block_size: self.device.block_size(),
            total_blocks: self.device.total_blocks(),
            free_blocks: sb.free_blocks_count() as usize,
            available_blocks: sb.free_blocks_count() as usize,
            total_inodes: sb.inodes_count as usize,
            free_inodes: sb.free_inodes_count() as usize,
            fsid: self.device.device_id() as u64,
            max_filename_len: 255, // EXT4_NAME_LEN
        })
    }
}
