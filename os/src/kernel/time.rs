//! 时间相关功能

use spin::RwLock;

use crate::{device::RTC_DRIVERS, pr_info, vfs::TimeSepc};

lazy_static::lazy_static! {
    /// 墙上时钟，记录自 1970-01-01 00:00:00 UTC 以来的时间（以秒为单位）
    /// XXX: 使用锁会不会影响精度？
    pub static ref REALTIME: RwLock<TimeSepc> = RwLock::new(TimeSepc::zero());
}

/// 初始化时间子系统
pub fn init() {
    // 初始化墙上时钟为 0
    pr_info!("Initializing REALTIME clock...");
    let mut realtime = REALTIME.write();
    let sec = RTC_DRIVERS
        .read()
        .first()
        .map(|rtc| rtc.read_epoch() as usize)
        .unwrap_or(0);
    let mtime = TimeSepc::monotonic_now();
    // 这里减去 mtime 是为简化后续的时间计算
    let time = TimeSepc::new(sec as i64, 0) - mtime;
    *realtime = time;
    pr_info!(
        "REALTIME clock initialized to {:?} seconds since epoch.",
        time
    );
}

pub fn update_realtime(time: &TimeSepc) {
    let mut realtime = REALTIME.write();
    *realtime = *time;
}
