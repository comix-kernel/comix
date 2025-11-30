//! 网络设备 sysfs 树构建器

use alloc::format;
use alloc::string::ToString;
use alloc::sync::Arc;

use crate::fs::sysfs::device_registry;
use crate::fs::sysfs::inode::{SysfsAttr, SysfsInode};
use crate::vfs::{FileMode, FsError, Inode};

/// 构建网络设备 sysfs 树
pub fn build_net_devices(root: &Arc<SysfsInode>) -> Result<(), FsError> {
    let class_inode = root.lookup("class")?;
    let class = class_inode.downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    let net_inode = class.lookup("net")?;
    let net_dir = net_inode.downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    for dev_info in device_registry::list_net_devices() {
        build_net_device(net_dir, &dev_info.name, dev_info.device)?;
    }

    Ok(())
}

fn build_net_device(
    parent: &SysfsInode,
    name: &str,
    device: Arc<dyn crate::device::net::net_device::NetDevice>,
) -> Result<(), FsError> {
    // 创建设备目录
    let dev_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));

    // address 文件: MAC 地址
    let address_attr = SysfsAttr {
        name: "address".to_string(),
        mode: FileMode::from_bits_truncate(0o444),
        show: {
            let dev = device.clone();
            Arc::new(move || {
                let mac = dev.mac_address();
                Ok(format!(
                    "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}\n",
                    mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
                ))
            })
        },
        store: None,
    };
    dev_dir.add_child("address", SysfsInode::new_attribute(address_attr))?;

    // mtu 文件
    let mtu_attr = SysfsAttr {
        name: "mtu".to_string(),
        mode: FileMode::from_bits_truncate(0o444), // 只读
        show: {
            let dev = device.clone();
            Arc::new(move || Ok(format!("{}\n", dev.mtu())))
        },
        store: None,
    };
    dev_dir.add_child("mtu", SysfsInode::new_attribute(mtu_attr))?;

    // operstate 文件
    let operstate_attr = SysfsAttr {
        name: "operstate".to_string(),
        mode: FileMode::from_bits_truncate(0o444),
        show: Arc::new(|| {
            // TODO: 从设备获取实际状态
            Ok("up\n".to_string())
        }),
        store: None,
    };
    dev_dir.add_child("operstate", SysfsInode::new_attribute(operstate_attr))?;

    // 添加到父目录
    parent.add_child(name, dev_dir)?;

    Ok(())
}
