//! 块设备 sysfs 树构建器
//!
//! 在 /sys/class/block/ 下创建指向 /sys/devices/platform/ 的符号链接

use alloc::format;
use alloc::sync::Arc;

use crate::fs::sysfs::device_registry;
use crate::fs::sysfs::inode::SysfsInode;
use crate::vfs::{FsError, Inode};

/// 构建块设备 sysfs 树
///
/// 在 /sys/class/block/ 下创建指向 /sys/devices/platform/<device>/ 的符号链接
pub fn build_block_devices(root: &Arc<SysfsInode>) -> Result<(), FsError> {
    // 获取 /sys/class/block/
    let class_inode = root.lookup("class")?;
    let class = class_inode
        .downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    let block_inode = class.lookup("block")?;
    let block_dir = block_inode
        .downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    // 为每个块设备创建符号链接
    for dev_info in device_registry::list_block_devices() {
        // 创建符号链接: /sys/class/block/vda -> ../../devices/platform/vda
        let target = format!("../../devices/platform/{}", dev_info.name);
        let symlink = SysfsInode::new_symlink(target);
        block_dir.add_child(&dev_info.name, symlink)?;
    }

    Ok(())
}
