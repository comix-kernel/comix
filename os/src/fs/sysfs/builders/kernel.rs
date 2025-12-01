//! 内核信息 sysfs 树构建器

use alloc::string::ToString;
use alloc::sync::Arc;

use crate::fs::sysfs::inode::{SysfsAttr, SysfsInode};
use crate::vfs::{FileMode, FsError, Inode};

/// 构建内核信息 sysfs 树
pub fn build_kernel_info(root: &Arc<SysfsInode>) -> Result<(), FsError> {
    let kernel_inode = root.lookup("kernel")?;
    let kernel_dir = kernel_inode
        .downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    // /sys/kernel/version
    let version_attr = SysfsAttr {
        name: "version".to_string(),
        mode: FileMode::from_bits_truncate(0o444),
        show: Arc::new(|| {
            // TODO: 从内核配置获取编译时间
            Ok("#1 SMP Mon Jan 1 00:00:00 UTC 2025\n".to_string())
        }),
        store: None,
    };
    kernel_dir.add_child("version", SysfsInode::new_attribute(version_attr))?;

    // /sys/kernel/osrelease
    let osrelease_attr = SysfsAttr {
        name: "osrelease".to_string(),
        mode: FileMode::from_bits_truncate(0o444),
        show: Arc::new(|| {
            // TODO: 从内核配置获取版本号
            Ok("0.1.0\n".to_string())
        }),
        store: None,
    };
    kernel_dir.add_child("osrelease", SysfsInode::new_attribute(osrelease_attr))?;

    Ok(())
}
