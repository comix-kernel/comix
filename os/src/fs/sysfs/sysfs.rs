//! Sysfs 文件系统实现
//!
//! 提供与 Linux 兼容的 sysfs 虚拟文件系统,暴露设备和内核信息。

use alloc::sync::Arc;

use crate::vfs::{FileMode, FileSystem, FsError, Inode, StatFs};

use super::inode::SysfsInode;

/// Sysfs 文件系统
pub struct SysFS {
    root_inode: Arc<SysfsInode>,
}

impl SysFS {
    /// 创建新的 sysfs 实例
    pub fn new() -> Arc<Self> {
        let root = SysfsInode::new_directory(FileMode::from_bits_truncate(
            0o040000 | 0o555, // dr-xr-xr-x
        ));

        Arc::new(Self { root_inode: root })
    }

    /// 初始化 sysfs 树结构 (冷插拔)
    pub fn init_tree(self: &Arc<Self>) -> Result<(), FsError> {
        // 创建基本目录结构
        self.create_directory_structure()?;

        // 从设备注册表构建设备树
        self.build_device_trees()?;

        Ok(())
    }

    fn create_directory_structure(&self) -> Result<(), FsError> {
        let root = &self.root_inode;

        // /sys/class/
        let class_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));
        root.add_child("class", class_dir.clone())?;

        // /sys/class/block/
        let block_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));
        class_dir.add_child("block", block_dir)?;

        // /sys/class/net/
        let net_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));
        class_dir.add_child("net", net_dir)?;

        // /sys/kernel/
        let kernel_dir =
            SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));
        root.add_child("kernel", kernel_dir)?;

        // /sys/devices/
        let devices_dir =
            SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));
        root.add_child("devices", devices_dir)?;

        Ok(())
    }

    fn build_device_trees(&self) -> Result<(), FsError> {
        use super::builders;

        // 构建块设备树
        builders::block::build_block_devices(&self.root_inode)?;

        // 构建网络设备树
        builders::net::build_net_devices(&self.root_inode)?;

        // 构建内核信息树
        builders::kernel::build_kernel_info(&self.root_inode)?;

        Ok(())
    }
}

impl FileSystem for SysFS {
    fn fs_type(&self) -> &'static str {
        "sysfs"
    }

    fn root_inode(&self) -> Arc<dyn Inode> {
        self.root_inode.clone()
    }

    fn sync(&self) -> Result<(), FsError> {
        // sysfs 是纯虚拟文件系统,无需同步
        Ok(())
    }

    fn statfs(&self) -> Result<StatFs, FsError> {
        Ok(StatFs {
            block_size: 4096,
            total_blocks: 0,
            free_blocks: 0,
            available_blocks: 0,
            total_inodes: 0,
            free_inodes: 0,
            fsid: 0,
            max_filename_len: 255,
        })
    }
}
