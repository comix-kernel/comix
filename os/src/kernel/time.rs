//! 

use core::sync::atomic::{AtomicUsize, Ordering};

use crate::{device::RTC_DRIVERS, pr_info};

/// 墙上时钟，记录自 1970-01-01 00:00:00 UTC 以来的时间（以秒为单位）
pub static REALTIME: AtomicUsize = AtomicUsize::new(0);

pub fn init() {
    // 初始化墙上时钟为 0
    pr_info!("Initializing REALTIME clock...");
    let time = RTC_DRIVERS
        .read()
        .first()
        .map(|rtc| rtc.read_epoch() as usize)
        .unwrap_or(0);
    REALTIME.store(time, Ordering::SeqCst);
    pr_info!("REALTIME clock initialized to {} seconds since epoch.", time);
}
