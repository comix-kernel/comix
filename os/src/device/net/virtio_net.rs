use virtio_drivers::transport::{mmio::MmioTransport, pci::PciTransport};

use crate::{
    device::{
        Driver,
        net::{add_network_device, net_device::VirtioNetDevice},
    },
    net::interface::NetworkInterface,
    pr_info, pr_warn, println,
    sync::SpinLock,
};
use alloc::{format, sync::Arc};
use lazy_static::lazy_static;

lazy_static! {
    static ref NET_DEVICE_COUNT: SpinLock<usize> = SpinLock::new(0);
}

pub fn init(transport: MmioTransport<'static>) {
    pr_info!("[Device] Initializing network driver (virtio-net)");

    // 获取设备ID
    let device_id = {
        let mut count = NET_DEVICE_COUNT.lock();
        let id = *count;
        *count += 1;
        id
    };
    pr_info!("[Device] Find VirtioNetDevice with ID: {}", device_id);

    // 创建VirtioNetDevice
    match VirtioNetDevice::new(transport, device_id) {
        Ok(virtio_device) => {
            pr_info!("[Device] VirtioNetDevice created with ID: {}", device_id);

            // 创建网络接口
            let interface_name = format!("eth{}", device_id);
            let network_interface =
                Arc::new(NetworkInterface::new(interface_name, virtio_device.clone()));

            // 将设备添加到全局设备列表
            add_network_device(virtio_device);

            // 将接口添加到全局接口管理器
            crate::net::interface::NETWORK_INTERFACE_MANAGER
                .lock()
                .add_interface(network_interface.clone());

            // 注册设备驱动
            crate::device::register_driver(network_interface.clone() as Arc<dyn Driver>);

            pr_info!(
                "[Device] Network interface {} initialized successfully",
                network_interface.name()
            );
        }
        Err(e) => {
            pr_warn!("[Device] Failed to initialize VirtioNetDevice: {:?}", e);
        }
    }
}

pub fn init_pci(transport: PciTransport) {
    pr_info!("[Device] Initializing network driver (virtio-net-pci)");

    let device_id = {
        let mut count = NET_DEVICE_COUNT.lock();
        let id = *count;
        *count += 1;
        id
    };
    pr_info!("[Device] Find VirtioNetDevice with ID: {}", device_id);

    match VirtioNetDevice::new(transport, device_id) {
        Ok(virtio_device) => {
            pr_info!("[Device] VirtioNetDevice created with ID: {}", device_id);

            let interface_name = format!("eth{}", device_id);
            let network_interface =
                Arc::new(NetworkInterface::new(interface_name, virtio_device.clone()));

            add_network_device(virtio_device);

            crate::net::interface::NETWORK_INTERFACE_MANAGER
                .lock()
                .add_interface(network_interface.clone());

            crate::device::register_driver(network_interface.clone() as Arc<dyn Driver>);

            pr_info!(
                "[Device] Network interface {} initialized successfully",
                network_interface.name()
            );
        }
        Err(e) => {
            pr_warn!("[Device] Failed to initialize VirtioNetDevice: {:?}", e);
        }
    }
}
