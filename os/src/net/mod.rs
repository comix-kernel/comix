//! 网络协议栈模块
//!
//! 提供网络接口管理、协议栈适配和网络配置功能

use alloc::{format, sync::Arc};

pub mod config;
pub mod interface;
pub mod socket;
pub mod stack;

/// Register a net device and create its current compatibility interface.
///
/// This is the migration boundary between device drivers and the network
/// subsystem. Interrupt compatibility is registered through `NetDriverHandle`
/// so `NetworkInterface` remains interface control-plane state.
pub fn register_net_device(
    device: Arc<dyn crate::device::net::net_device::NetDevice>,
) -> Arc<interface::NetworkInterface> {
    let interface_name = format!("eth{}", device.device_id());
    let network_interface = Arc::new(interface::NetworkInterface::new(
        interface_name,
        device.clone(),
    ));

    crate::device::net::add_network_device(device);
    interface::NETWORK_INTERFACE_MANAGER
        .lock()
        .add_interface(network_interface.clone());
    let driver = Arc::new(interface::NetDriverHandle::new(network_interface.clone()));
    crate::device::register_driver(driver as Arc<dyn crate::device::Driver>);

    network_interface
}
