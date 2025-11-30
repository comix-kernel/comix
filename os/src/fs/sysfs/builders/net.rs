//! 网络设备 sysfs 树构建器
//!
//! 在 /sys/class/net/ 下创建指向 /sys/devices/platform/ 的符号链接

use alloc::format;
use alloc::sync::Arc;

use crate::fs::sysfs::device_registry;
use crate::fs::sysfs::inode::SysfsInode;
use crate::vfs::{FsError, Inode};

/// 构建网络设备 sysfs 树
///
/// 在 /sys/class/net/ 下创建指向 /sys/devices/platform/<device>/ 的符号链接
pub fn build_net_devices(root: &Arc<SysfsInode>) -> Result<(), FsError> {
    let class_inode = root.lookup("class")?;
    let class = class_inode
        .downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    let net_inode = class.lookup("net")?;
    let net_dir = net_inode
        .downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    // 为每个网络设备创建符号链接
    for dev_info in device_registry::list_net_devices() {
        // 创建符号链接: /sys/class/net/eth0 -> ../../devices/platform/eth0
        let target = format!("../../devices/platform/{}", dev_info.name);
        let symlink = SysfsInode::new_symlink(target);
        net_dir.add_child(&dev_info.name, symlink)?;
    }

    Ok(())
}
