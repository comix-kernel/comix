use virtio_drivers::transport::{self, mmio::MmioTransport};

use crate::println;

pub fn init(transport: MmioTransport<'static>) {
    println!("[Device] Input driver (virtio-input) is initialized");
}
