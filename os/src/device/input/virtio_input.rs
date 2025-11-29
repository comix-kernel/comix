use virtio_drivers::transport::mmio::MmioTransport;

use crate::earlyprintln;

pub fn init(transport: MmioTransport<'static>) {
    earlyprintln!("[Device] Input driver (virtio-input) is initialized");
}
