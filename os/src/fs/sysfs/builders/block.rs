//! 块设备 sysfs 树构建器

use alloc::format;
use alloc::string::ToString;
use alloc::sync::Arc;

use crate::fs::sysfs::device_registry;
use crate::fs::sysfs::inode::{SysfsAttr, SysfsInode};
use crate::vfs::{FileMode, FsError, Inode};

/// 构建块设备 sysfs 树
pub fn build_block_devices(root: &Arc<SysfsInode>) -> Result<(), FsError> {
    // 获取 /sys/class/block/
    let class_inode = root.lookup("class")?;
    let class = class_inode.downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    let block_inode = class.lookup("block")?;
    let block_dir = block_inode.downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    // 遍历所有块设备
    for dev_info in device_registry::list_block_devices() {
        build_block_device(block_dir, &dev_info.name, dev_info.major, dev_info.minor, dev_info.device)?;
    }

    Ok(())
}

fn build_block_device(
    parent: &SysfsInode,
    name: &str,
    major: u32,
    minor: u32,
    device: Arc<dyn crate::device::block::BlockDriver>,
) -> Result<(), FsError> {
    // 创建设备目录 /sys/class/block/vda/
    let dev_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));

    // dev 文件: major:minor
    let dev_attr = SysfsAttr {
        name: "dev".to_string(),
        mode: FileMode::from_bits_truncate(0o444),
        show: {
            let maj = major;
            let min = minor;
            Arc::new(move || Ok(format!("{}:{}\n", maj, min)))
        },
        store: None,
    };
    dev_dir.add_child("dev", SysfsInode::new_attribute(dev_attr))?;

    // size 文件: 扇区数 (512 字节)
    let size_attr = SysfsAttr {
        name: "size".to_string(),
        mode: FileMode::from_bits_truncate(0o444),
        show: {
            let dev = device.clone();
            Arc::new(move || {
                // 计算总扇区数 (512 字节/扇区)
                let block_size = dev.block_size();
                let total_blocks = dev.total_blocks();
                let total_bytes = block_size * total_blocks;
                let sectors = total_bytes / 512;
                Ok(format!("{}\n", sectors))
            })
        },
        store: None,
    };
    dev_dir.add_child("size", SysfsInode::new_attribute(size_attr))?;

    // ro 文件: 是否只读
    let ro_attr = SysfsAttr {
        name: "ro".to_string(),
        mode: FileMode::from_bits_truncate(0o444),
        show: Arc::new(|| Ok("0\n".to_string())), // 假设不只读
        store: None,
    };
    dev_dir.add_child("ro", SysfsInode::new_attribute(ro_attr))?;

    // removable 文件: 是否可移动
    let removable_attr = SysfsAttr {
        name: "removable".to_string(),
        mode: FileMode::from_bits_truncate(0o444),
        show: Arc::new(|| Ok("0\n".to_string())), // 冷插拔,不可移动
        store: None,
    };
    dev_dir.add_child("removable", SysfsInode::new_attribute(removable_attr))?;

    // 添加到父目录
    parent.add_child(name, dev_dir)?;

    Ok(())
}
