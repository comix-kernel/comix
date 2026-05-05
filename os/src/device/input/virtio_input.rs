use virtio_drivers::transport::mmio::MmioTransport;

use crate::pr_info;

pub fn init(_transport: MmioTransport<'static>) {
    pr_info!("[Device] Input driver (virtio-input) is initialized");
}
