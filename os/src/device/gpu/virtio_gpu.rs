use virtio_drivers::transport::mmio::MmioTransport;

use crate::println;

pub fn init(transport: MmioTransport<'static>) {
    println!("[Device] GPU driver (virtio-gpu) is initialized");
}
