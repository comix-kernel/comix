use virtio_drivers::transport::{mmio::MmioTransport, pci::PciTransport};

use crate::{
    device::net::net_device::{NetDeviceError, VirtioNetDevice},
    pr_info, pr_warn,
    sync::SpinLock,
};
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
            let network_interface = crate::net::register_net_device(virtio_device);

            pr_info!(
                "[Device] Network interface {} initialized successfully",
                network_interface.name()
            );
        }
        Err(NetDeviceError::DeviceNotReady) => {
            pr_info!("[Device] VirtioNetDevice not ready; skipping");
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
            let network_interface = crate::net::register_net_device(virtio_device);

            pr_info!(
                "[Device] Network interface {} initialized successfully",
                network_interface.name()
            );
        }
        Err(NetDeviceError::DeviceNotReady) => {
            pr_info!("[Device] VirtioNetDevice not ready; skipping");
        }
        Err(e) => {
            pr_warn!("[Device] Failed to initialize VirtioNetDevice: {:?}", e);
        }
    }
}
