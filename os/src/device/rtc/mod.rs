//! RTC 设备驱动模块

use super::Driver;

pub mod rtc_goldfish;

/// RTC 设备驱动接口
pub trait RtcDriver: Driver {
    /// 读取自纪元以来的秒数
    fn read_epoch(&self) -> u64;
}
