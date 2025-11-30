//! 设备注册表辅助 - 为 sysfs 提供设备信息访问
//!
//! 这个模块不创建新的设备注册表,而是为 sysfs 提供访问现有设备注册表的辅助函数。

use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::device::{BLK_DRIVERS, DRIVERS, DeviceType};
use crate::device::block::BlockDriver;
use crate::device::net::net_device::NetDevice;

/// 块设备信息 (用于 sysfs)
#[derive(Clone)]
pub struct BlockDeviceInfo {
    pub name: String,
    pub major: u32,
    pub minor: u32,
    pub device: Arc<dyn BlockDriver>,
}

/// 网络设备信息 (用于 sysfs)
#[derive(Clone)]
pub struct NetworkDeviceInfo {
    pub name: String,
    pub device: Arc<dyn NetDevice>,
}

/// 列出所有块设备
pub fn list_block_devices() -> Vec<BlockDeviceInfo> {
    let drivers = BLK_DRIVERS.read();
    drivers
        .iter()
        .enumerate()
        .map(|(idx, driver)| {
            // 生成设备名: vda, vdb, vdc...
            let name = format!("vd{}", (b'a' + idx as u8) as char);
            // VirtIO 块设备的主设备号通常是 254
            let major = 254;
            let minor = idx as u32;

            BlockDeviceInfo {
                name,
                major,
                minor,
                device: driver.clone(),
            }
        })
        .collect()
}

/// 列出所有网络设备
pub fn list_net_devices() -> Vec<NetworkDeviceInfo> {
    let drivers = DRIVERS.read();
    drivers
        .iter()
        .filter(|driver| driver.device_type() == DeviceType::Net)
        .filter_map(|driver| {
            driver.as_net().map(|net_dev| {
                NetworkDeviceInfo {
                    name: net_dev.name().to_string(),
                    device: unsafe {
                        // SAFETY: 我们从 Arc<dyn Driver> 获取 &dyn NetDevice,
                        // 需要重新包装为 Arc<dyn NetDevice>
                        // 这里使用 transmute 来实现类型转换
                        core::mem::transmute::<Arc<dyn crate::device::Driver>, Arc<dyn NetDevice>>(
                            Arc::clone(driver)
                        )
                    },
                }
            })
        })
        .collect()
}

/// 根据名称查找块设备
pub fn find_block_device(name: &str) -> Option<BlockDeviceInfo> {
    list_block_devices().into_iter().find(|dev| dev.name == name)
}

/// 根据名称查找网络设备
pub fn find_net_device(name: &str) -> Option<NetworkDeviceInfo> {
    list_net_devices().into_iter().find(|dev| dev.name == name)
}
