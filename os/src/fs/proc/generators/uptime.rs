use alloc::format;
use alloc::vec::Vec;

use crate::fs::proc::inode::ContentGenerator;
use crate::vfs::FsError;

pub struct UptimeGenerator;

impl ContentGenerator for UptimeGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        // 获取系统运行时间（秒）
        let uptime_ms = crate::arch::timer::get_time_ms();
        let uptime_sec = uptime_ms / 1000;
        let uptime_frac = (uptime_ms % 1000) / 10; // 保留2位小数

        // TODO: 获取空闲时间
        let idle_sec = 0;
        let idle_frac = 0;

        let content = format!(
            "{}.{:02} {}.{:02}\n",
            uptime_sec, uptime_frac, idle_sec, idle_frac
        );

        Ok(content.into_bytes())
    }
}
