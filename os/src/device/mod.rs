//! 设备抽象层，提供块设备接口、内存磁盘实现和网络设备接口

#[macro_use]
pub mod bus;
pub mod block;
pub mod console;
pub mod gpu;
pub mod input;
pub mod irq;
pub mod net;
pub mod rtc;
pub mod serial;
pub mod virtio_hal;

pub mod device_tree;

use crate::sync::RwLock;
use alloc::sync::Arc;
pub use block::ram_disk::RamDisk;

use crate::device::rtc::RtcDriver;

use crate::device::serial::SerialDriver;
use crate::device::{block::BlockDriver, net::net_device::NetDevice};

use alloc::{string::String, vec::Vec};
use lazy_static::lazy_static;

/// 设备类型枚举
#[derive(Debug, Eq, PartialEq)]
pub enum DeviceType {
    /// 网络设备
    Net,
    /// 图形处理单元设备
    Gpu,
    /// 输入设备
    Input,
    /// 块设备
    Block,
    /// 实时时钟设备
    Rtc,
    /// 串行设备
    Serial,
    /// 中断控制器
    Intc,
}

/// 设备驱动程序特征
pub trait Driver: Send + Sync {
    // 如果中断属于此驱动程序，则处理它并返回 true
    // 否则返回 false
    // 中断号在可用时提供
    // 如果中断号不匹配，驱动程序应跳过处理。
    fn try_handle_interrupt(&self, irq: Option<usize>) -> bool;

    // 返回对应的设备类型，请参阅 DeviceType
    fn device_type(&self) -> DeviceType;

    // 获取此设备的唯一标识符
    // 每个实例的标识符应该不同
    fn get_id(&self) -> String;

    /// 将驱动程序转换为网络驱动程序（如果适用）
    fn as_net(&self) -> Option<&dyn NetDevice> {
        None
    }

    /// 将驱动程序转换为网络驱动程序 Arc（如果适用）
    fn as_net_arc(self: Arc<Self>) -> Option<Arc<dyn NetDevice>> {
        None
    }

    /// 将驱动程序转换为块设备驱动程序（如果适用）
    fn as_block(&self) -> Option<&dyn BlockDriver> {
        None
    }

    /// 将驱动程序转换为块设备驱动程序 Arc（如果适用）
    fn as_block_arc(self: Arc<Self>) -> Option<Arc<dyn BlockDriver>> {
        None
    }

    /// 将驱动程序转换为实时时钟驱动程序（如果适用）
    fn as_rtc(&self) -> Option<&dyn RtcDriver> {
        None
    }

    /// 将驱动程序转换为实时时钟驱动程序 Arc（如果适用）
    fn as_rtc_arc(self: Arc<Self>) -> Option<Arc<dyn RtcDriver>> {
        None
    }

    /// 将驱动程序转换为串口驱动程序（如果适用）
    fn as_serial(&self) -> Option<&dyn serial::SerialDriver> {
        None
    }
}

lazy_static! {
    // NOTE: RwLock 只在初始化阶段有写操作，运行时均为读操作
    pub static ref DRIVERS: RwLock<Vec<Arc<dyn Driver>>> = RwLock::new(Vec::new());
    pub static ref BLK_DRIVERS: RwLock<Vec<Arc<dyn BlockDriver>>> = RwLock::new(Vec::new());
    pub static ref RTC_DRIVERS: RwLock<Vec<Arc<dyn RtcDriver>>> = RwLock::new(Vec::new());
    pub static ref SERIAL_DRIVERS: RwLock<Vec<Arc<dyn SerialDriver>>> = RwLock::new(Vec::new());
    pub static ref IRQ_MANAGER: RwLock<irq::IrqManager> = RwLock::new(irq::IrqManager::new(true));
}

lazy_static! {
    // 内核命令行参数
    // 存储从设备树中提取的 bootargs 属性
    pub static ref CMDLINE: RwLock<String> = RwLock::new(String::new());
}

/// 注册设备驱动
pub fn register_driver(driver: Arc<dyn Driver>) {
    DRIVERS.write().push(driver);
}
