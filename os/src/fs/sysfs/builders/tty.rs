//! TTY 设备 sysfs 树构建器
//!
//! 在 /sys/class/tty/ 下创建指向 /sys/devices/platform/ 的符号链接

use alloc::format;
use alloc::sync::Arc;

use crate::fs::sysfs::device_registry;
use crate::fs::sysfs::inode::SysfsInode;
use crate::vfs::{FsError, Inode};

/// 构建 TTY 设备 sysfs 树
///
/// 在 /sys/class/tty/ 下创建指向 /sys/devices/platform/<device>/ 的符号链接
pub fn build_tty_devices(root: &Arc<SysfsInode>) -> Result<(), FsError> {
    let class_inode = root.lookup("class")?;
    let class = class_inode
        .downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    let tty_inode = class.lookup("tty")?;
    let tty_dir = tty_inode
        .downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    // 为每个 TTY 设备创建符号链接
    for dev_info in device_registry::list_tty_devices() {
        // 创建符号链接: /sys/class/tty/console -> ../../devices/platform/console
        let target = format!("../../devices/platform/{}", dev_info.name);
        let symlink = SysfsInode::new_symlink(target);
        tty_dir.add_child(&dev_info.name, symlink)?;
    }

    Ok(())
}
