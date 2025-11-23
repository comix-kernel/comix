use virtio_drivers::transport::mmio::MmioTransport;

use crate::println;

pub fn init(transport: MmioTransport<'static>) {
    println!("[Device] Network driver (virtio-net) is initialized");
}
