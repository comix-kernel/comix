use virtio_drivers::transport::mmio::MmioTransport;

use crate::earlyprintln;

pub fn init(transport: MmioTransport<'static>) {
    earlyprintln!("[Device] GPU driver (virtio-gpu) is initialized");
}
