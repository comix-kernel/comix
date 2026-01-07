//! 网络设备驱动模块
//!
//! 管理和初始化各种网络设备

use crate::device::NetDevice;
use crate::sync::SpinLock;
use alloc::{sync::Arc, vec::Vec};

pub mod net_device;
pub mod null_net;
pub mod virtio_net;

use lazy_static::lazy_static;
lazy_static! {
    /// 网络设备管理器
    /// 负责存储和管理系统中的所有网络设备
    pub static ref NETWORK_DEVICES: SpinLock<Vec<Arc<dyn NetDevice>>> = SpinLock::new(Vec::new());
}

/// 添加网络设备到网络设备管理器
pub fn add_network_device(device: Arc<dyn NetDevice>) {
    NETWORK_DEVICES.lock().push(device);
}
/// 获取所有网络设备
pub fn get_net_devices() -> Vec<alloc::sync::Arc<dyn NetDevice>> {
    NETWORK_DEVICES.lock().clone()
}

/// 格式化MAC地址为可读字符串
fn format_mac_address(mac: [u8; 6]) -> alloc::string::String {
    use alloc::format;
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}
