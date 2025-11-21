use alloc::sync::Arc;
use alloc::{format, string::String};
use virtio_drivers::device::blk::VirtIOBlk;
use virtio_drivers::transport::mmio::MmioTransport;

use crate::device::net::NetDriver;
use crate::device::virtio_hal::VirtIOHal;
use crate::device::{BLK_DRIVERS, DRIVERS};
use crate::println;
use crate::sync::Mutex;

use super::{
    super::{DeviceType, Driver},
    BlockDriver,
};

struct VirtIOBlkDriver(Mutex<VirtIOBlk<VirtIOHal, MmioTransport<'static>>>);

impl Driver for VirtIOBlkDriver {
    fn try_handle_interrupt(&self, _irq: Option<usize>) -> bool {
        self.0.lock().ack_interrupt();
        true
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Block
    }

    fn get_id(&self) -> String {
        format!("virtio_block")
    }

    fn as_block(&self) -> Option<&dyn BlockDriver> {
        Some(self)
    }

    fn as_net(&self) -> Option<&dyn NetDriver> {
        None
    }

    fn as_rtc(&self) -> Option<&dyn crate::device::rtc::RtcDriver> {
        None
    }
}

impl BlockDriver for VirtIOBlkDriver {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> bool {
        self.0.lock().read_blocks(block_id, buf).is_ok()
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) -> bool {
        self.0.lock().write_blocks(block_id, buf).is_ok()
    }
}

pub fn init(transport: MmioTransport<'static>) {
    let blk = VirtIOBlk::new(transport).expect("failed to init blk driver");
    let driver = Arc::new(VirtIOBlkDriver(Mutex::new(blk)));
    DRIVERS.write().push(driver.clone());
    // IRQ_MANAGER.write().register_all(driver.clone());
    BLK_DRIVERS.write().push(driver);
    println!("[Device] Block driver (virtio-blk) is initialized");
}
