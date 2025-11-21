//! 设备抽象层，提供块设备接口、内存磁盘实现和网络设备接口

#[macro_use]
pub mod bus;
pub mod block;
pub mod gpu;
pub mod input;
pub mod net;
pub mod rtc;
pub mod virtio_hal;

pub mod device_tree;

use alloc::sync::Arc;
pub use block::block_device::BlockDevice;
pub use block::ram_disk::RamDisk;
pub use net::net_device::NetDevice;
use spin::RwLock;
use virtio_drivers::transport::DeviceType;

use crate::device::rtc::RtcDriver;
use crate::device::{block::BlockDriver, net::NetDriver};
use crate::sync::SpinLock;
use alloc::{string::String, vec::Vec};
use lazy_static::lazy_static;

pub trait Driver: Send + Sync {
    // if interrupt belongs to this driver, handle it and return true
    // return false otherwise
    // irq number is provided when available
    // driver should skip handling when irq number is mismatched
    fn try_handle_interrupt(&self, irq: Option<usize>) -> bool;

    // return the correspondent device type, see DeviceType
    fn device_type(&self) -> DeviceType;

    // get unique identifier for this device
    // should be different for each instance
    fn get_id(&self) -> String;

    // trait casting
    fn as_net(&self) -> Option<&dyn NetDriver> {
        None
    }

    fn as_block(&self) -> Option<&dyn BlockDriver> {
        None
    }

    fn as_rtc(&self) -> Option<&dyn RtcDriver> {
        None
    }
}

pub fn init() {
    device_tree::init();
    net::init_net_devices();
}

lazy_static! {
    /// 网络设备管理器
    /// 负责存储和管理系统中的所有网络设备
    /// FIXME: 尽快迁移到 NET_DRIVERS 之后此结构将废弃
    pub static ref NETWORK_DEVICES: SpinLock<Vec<Arc<dyn NetDevice>>> = SpinLock::new(Vec::new());
}

lazy_static! {
    // NOTE: RwLock 只在初始化阶段有写操作，运行时均为读操作
    pub static ref DRIVERS: RwLock<Vec<Arc<dyn Driver>>> = RwLock::new(Vec::new());
    pub static ref NET_DRIVERS: RwLock<Vec<Arc<dyn NetDriver>>> = RwLock::new(Vec::new());
    pub static ref BLK_DRIVERS: RwLock<Vec<Arc<dyn BlockDriver>>> = RwLock::new(Vec::new());
    pub static ref RTC_DRIVERS: RwLock<Vec<Arc<dyn RtcDriver>>> = RwLock::new(Vec::new());
    // pub static ref SERIAL_DRIVERS: RwLock<Vec<Arc<dyn SerialDriver>>> = RwLock::new(Vec::new());
    // pub static ref IRQ_MANAGER: RwLock<irq::IrqManager> = RwLock::new(irq::IrqManager::new(true));
}
