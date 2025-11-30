//! RTC 设备驱动模块

use super::Driver;
use chrono::{DateTime as ChronoDateTime, Datelike, FixedOffset, TimeZone, Timelike, Utc};

/// 简化的日期时间结构（用于 sysfs 显示）
#[derive(Debug, Clone, Copy)]
pub struct DateTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl DateTime {
    /// 从 Unix 时间戳(秒)转换为北京时间 (UTC+8)
    pub fn from_epoch(epoch: u64) -> Self {
        // 创建 UTC 时间
        let utc_time = match Utc.timestamp_opt(epoch as i64, 0) {
            chrono::LocalResult::Single(t) => t,
            _ => {
                // 如果时间戳无效，返回一个默认值
                return Self {
                    year: 1970,
                    month: 1,
                    day: 1,
                    hour: 0,
                    minute: 0,
                    second: 0,
                };
            }
        };

        // 转换为北京时间 (UTC+8)
        let beijing_offset = FixedOffset::east_opt(8 * 3600).unwrap();
        let beijing_time: ChronoDateTime<FixedOffset> = utc_time.with_timezone(&beijing_offset);

        Self {
            year: beijing_time.year(),
            month: beijing_time.month(),
            day: beijing_time.day(),
            hour: beijing_time.hour(),
            minute: beijing_time.minute(),
            second: beijing_time.second(),
        }
    }
}

pub mod rtc_goldfish;

/// RTC 设备驱动接口
pub trait RtcDriver: Driver {
    /// 读取自纪元以来的秒数
    fn read_epoch(&self) -> u64;

    /// 读取日期时间（北京时间，默认实现）
    fn read_datetime(&self) -> DateTime {
        DateTime::from_epoch(self.read_epoch())
    }
}
