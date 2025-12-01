//! RTC 设备 sysfs 树构建器
//!
//! 在 /sys/class/rtc/ 下创建指向 /sys/devices/platform/ 的符号链接

use alloc::format;
use alloc::sync::Arc;

use crate::fs::sysfs::device_registry;
use crate::fs::sysfs::inode::SysfsInode;
use crate::vfs::{FsError, Inode};

/// 构建 RTC 设备 sysfs 树
///
/// 在 /sys/class/rtc/ 下创建指向 /sys/devices/platform/<device>/ 的符号链接
pub fn build_rtc_devices(root: &Arc<SysfsInode>) -> Result<(), FsError> {
    let class_inode = root.lookup("class")?;
    let class = class_inode
        .downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    let rtc_inode = class.lookup("rtc")?;
    let rtc_dir = rtc_inode
        .downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    // 为每个 RTC 设备创建符号链接
    for dev_info in device_registry::list_rtc_devices() {
        // 创建符号链接: /sys/class/rtc/rtc0 -> ../../devices/platform/rtc0
        let target = format!("../../devices/platform/{}", dev_info.name);
        let symlink = SysfsInode::new_symlink(target);
        rtc_dir.add_child(&dev_info.name, symlink)?;
    }

    Ok(())
}
