//! 定义与时间相关的结构和常量。
//!
//! 这些定义用于系统调用如 `clock_gettime`, `nanosleep`, `timer_create` 等。

#![allow(non_camel_case_types)]
#![allow(dead_code)]

use core::{
    ffi::{c_int, c_long},
    ops::{Add, Sub},
};

use crate::{
    arch::timer::{clock_freq, get_time},
    kernel::time::REALTIME,
};

/// 用于指定秒和纳秒精度的时间
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeSpec {
    /// 秒 (seconds)
    pub tv_sec: c_long,
    /// 纳秒 (nanoseconds)
    pub tv_nsec: c_long,
}

impl TimeSpec {
    /// 特殊值：设置为当前时间（UTIME_NOW，用于 utimensat）
    pub const UTIME_NOW: c_long = (1i64 << 30) - 1; // 1073741823

    /// 特殊值：不改变此时间（UTIME_OMIT，用于 utimensat）
    pub const UTIME_OMIT: c_long = (1i64 << 30) - 2; // 1073741822

    /// 创建一个新的 TimeSpec 结构体
    /// # 参数:
    /// - `sec`: 秒数
    /// - `nsec`: 纳秒数
    /// # 返回值:
    /// - 对应的 TimeSpec 结构体
    pub fn new(sec: c_long, nsec: c_long) -> Self {
        Self {
            tv_sec: sec,
            tv_nsec: nsec,
        }
    }

    /// 将 TimeSpec 转换为指定频率的刻度数。
    /// # 参数:
    /// - `freq`: 频率（每秒刻度数）
    /// # 返回值:
    /// - 刻度数
    pub fn into_freq(&self, freq: usize) -> usize {
        let sec_ticks = (self.tv_sec as u128) * (freq as u128);
        let nsec_ticks = (self.tv_nsec as u128) * (freq as u128) / 1_000_000_000;
        (sec_ticks + nsec_ticks) as usize
    }

    /// 通过指定频率的刻度数创建 TimeSpec。
    /// # 参数:
    /// - `ticks`: 刻度数
    /// - `freq`: 频率（每秒刻度数）
    /// # 返回值:
    /// - 对应的 TimeSpec 结构体
    pub fn from_freq(ticks: usize, freq: usize) -> Self {
        let sec = ticks / freq;
        let nsec = (ticks % freq) * 1_000_000_000 / freq;
        Self {
            tv_sec: sec as c_long,
            tv_nsec: nsec as c_long,
        }
    }

    /// 获取当前墙上时钟时间的 TimeSpec。
    /// # 返回值:
    /// - 当前时间的 TimeSpec 结构体
    pub fn now() -> Self {
        let time = REALTIME.read();
        let mtime = Self::monotonic_now();
        mtime + *time
    }

    /// 获取当前单调时钟时间的 TimeSpec。
    /// # 返回值:
    /// - 当前单调时间的 TimeSpec 结构体
    pub fn monotonic_now() -> Self {
        let time = get_time();
        Self::from_freq(time, clock_freq())
    }

    /// 创建零时间的 TimeSpec。
    /// # 返回值:
    /// - 零时间的 TimeSpec 结构体
    pub fn zero() -> Self {
        Self {
            tv_sec: 0,
            tv_nsec: 0,
        }
    }

    /// 验证 TimeSpec 的有效性（用于 utimensat）
    ///
    /// # 返回值
    /// - `Ok(())`: 有效
    /// - `Err(EINVAL)`: 无效
    pub fn validate(&self) -> Result<(), i32> {
        use crate::uapi::errno::EINVAL;

        // 检查特殊值
        if self.tv_nsec == Self::UTIME_NOW || self.tv_nsec == Self::UTIME_OMIT {
            return Ok(());
        }

        // 检查纳秒范围
        if self.tv_nsec < 0 || self.tv_nsec >= 1_000_000_000 {
            return Err(EINVAL);
        }

        Ok(())
    }

    /// 将 TimeSpec 转换为 timeval 结构体。
    /// # 返回值:
    /// - 对应的 timeval 结构体
    pub fn to_timeval(&self) -> timeval {
        timeval {
            tv_sec: self.tv_sec,
            tv_usec: self.tv_nsec / 1000,
        }
    }

    /// 检查是否为零时间
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.tv_sec == 0 && self.tv_nsec == 0
    }

    /// 检查是否为 UTIME_NOW（用于 utimensat）
    #[inline]
    pub fn is_now(&self) -> bool {
        self.tv_nsec == Self::UTIME_NOW
    }

    /// 检查是否为 UTIME_OMIT（用于 utimensat）
    #[inline]
    pub fn is_omit(&self) -> bool {
        self.tv_nsec == Self::UTIME_OMIT
    }
}

impl Sub for TimeSpec {
    type Output = TimeSpec;

    fn sub(self, other: TimeSpec) -> TimeSpec {
        let sec = self.tv_sec - other.tv_sec;
        let nsec = self.tv_nsec - other.tv_nsec;
        if nsec < 0 {
            TimeSpec {
                tv_sec: sec - 1,
                tv_nsec: nsec + 1_000_000_000,
            }
        } else {
            TimeSpec {
                tv_sec: sec,
                tv_nsec: nsec,
            }
        }
    }
}

impl Add for TimeSpec {
    type Output = TimeSpec;

    fn add(self, other: TimeSpec) -> TimeSpec {
        let sec = self.tv_sec + other.tv_sec;
        let nsec = self.tv_nsec + other.tv_nsec;
        if nsec >= 1_000_000_000 {
            TimeSpec {
                tv_sec: sec + 1,
                tv_nsec: nsec - 1_000_000_000,
            }
        } else {
            TimeSpec {
                tv_sec: sec,
                tv_nsec: nsec,
            }
        }
    }
}

/// 用于指定秒和微秒精度的时间。
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct timeval {
    /// 秒 (seconds)
    pub tv_sec: c_long,
    /// 微秒 (microseconds)
    pub tv_usec: c_long,
}

impl timeval {
    /// 将 timeval 转换为 TimeSpec 结构体。
    /// # 返回值:
    /// - 对应的 TimeSpec 结构体
    pub fn to_timespec(&self) -> TimeSpec {
        TimeSpec {
            tv_sec: self.tv_sec,
            tv_nsec: self.tv_usec * 1000,
        }
    }

    /// 将 timeval 转换为指定频率的刻度数。
    /// # 参数:
    /// - `freq`: 频率（每秒刻度数）
    /// # 返回值:
    /// - 刻度数
    pub fn into_freq(&self, freq: usize) -> usize {
        let sec_ticks = (self.tv_sec as u128) * (freq as u128);
        let usec_ticks = (self.tv_usec as u128) * (freq as u128) / 1_000_000;
        (sec_ticks + usec_ticks) as usize
    }

    /// 创建一个新的 timeval 结构体
    /// # 参数:
    /// - `sec`: 秒数
    /// - `usec`: 微秒数
    /// # 返回值:
    /// - 对应的 timeval 结构体
    pub fn new(sec: c_long, usec: c_long) -> Self {
        Self {
            tv_sec: sec,
            tv_usec: usec,
        }
    }

    /// 创建零时间的 timeval 结构体
    /// # 返回值:
    /// - 零时间的 timeval 结构体
    pub fn zero() -> Self {
        Self {
            tv_sec: 0,
            tv_usec: 0,
        }
    }

    /// 检查是否为零时间
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.tv_sec == 0 && self.tv_usec == 0
    }
}

/// 用于设置 POSIX 间隔定时器 (timer_create) 的结构。
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Itimerspec {
    /// 定时器周期 (timer period)
    pub it_interval: TimeSpec,
    /// 定时器初始值/到期时间 (timer expiration)
    pub it_value: TimeSpec,
}

/// 用于设置传统 BSD 间隔定时器 (setitimer) 的结构。
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Itimerval {
    /// 定时器周期 (timer interval)
    pub it_interval: timeval,
    /// 定时器当前值 (current value)
    pub it_value: timeval,
}

impl Itimerval {
    /// 将 Itimerval 转换为 Itimerspec。
    /// # 返回值:
    /// - 对应的 Itimerspec 结构体
    pub fn zero() -> Self {
        Self {
            it_interval: timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            it_value: timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
        }
    }
}

/// 时区结构体，用于 gettimeofday/settimeofday（现在已不推荐使用）。
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct timezone {
    /// 格林威治以西的分钟数 (minutes west of Greenwich)
    pub tz_minuteswest: c_int,
    /// DST 校正类型 (type of dst correction)
    pub tz_dsttime: c_int,
}

/// 传统 BSD 风格间隔定时器 ID。
pub mod itimer_id {
    use super::c_int;
    /// 实时计时器。当计时器到期时发送 SIGALRM。
    pub const ITIMER_REAL: c_int = 0;
    /// 虚拟计时器。当进程处于用户态执行时计时，到期时发送 SIGVTALRM。
    pub const ITIMER_VIRTUAL: c_int = 1;
    /// 性能计时器。当进程处于用户态和内核态执行时计时，到期时发送 SIGPROF。
    pub const ITIMER_PROF: c_int = 2;
}

/// 用于 `clock_gettime`, `nanosleep` 等系统调用的时钟 ID。
pub mod clock_id {
    use super::c_int;

    /// 实时时钟，可被修改，非单调。
    pub const CLOCK_REALTIME: c_int = 0;
    /// 单调时钟，从系统启动开始计数，不计入休眠时间。
    pub const CLOCK_MONOTONIC: c_int = 1;
    /// 当前进程消耗的 CPU 时间。
    pub const CLOCK_PROCESS_CPUTIME_ID: c_int = 2;
    /// 当前线程消耗的 CPU 时间。
    pub const CLOCK_THREAD_CPUTIME_ID: c_int = 3;
    /// 原始单调时钟，未经 NTP 或频率调整。
    pub const CLOCK_MONOTONIC_RAW: c_int = 4;
    /// 粗粒度实时时钟，访问速度快。
    pub const CLOCK_REALTIME_COARSE: c_int = 5;
    /// 粗粒度单调时钟，访问速度快。
    pub const CLOCK_MONOTONIC_COARSE: c_int = 6;
    /// 启动时间时钟，包含系统休眠时间。
    pub const CLOCK_BOOTTIME: c_int = 7;
    /// 实时闹钟，即使系统休眠也会唤醒。
    pub const CLOCK_REALTIME_ALARM: c_int = 8;
    /// 启动时间闹钟，即使系统休眠也会唤醒。
    pub const CLOCK_BOOTTIME_ALARM: c_int = 9;
    /// SGI 循环时钟 (已移除，仅占位)。
    pub const CLOCK_SGI_CYCLE: c_int = 10;
    /// 国际原子时 (TAI)。
    pub const CLOCK_TAI: c_int = 11;

    // 最大时钟 ID 数量（用于数组或边界检查）
    pub const MAX_CLOCKS: c_int = 16;
}

/// 辅助时钟和标志
pub mod clock_flags {
    use crate::uapi::time::clock_id::{CLOCK_MONOTONIC, CLOCK_REALTIME};

    use super::c_int;

    // 辅助时钟的基数和范围
    pub const CLOCK_AUX: c_int = super::clock_id::MAX_CLOCKS;
    pub const MAX_AUX_CLOCKS: c_int = 8;
    pub const CLOCK_AUX_LAST: c_int = CLOCK_AUX + MAX_AUX_CLOCKS - 1;

    // 掩码，用于组合时钟 ID
    pub const CLOCKS_MASK: c_int = CLOCK_REALTIME | CLOCK_MONOTONIC;
    pub const CLOCKS_MONO: c_int = CLOCK_MONOTONIC;

    // POSIX.1b 定时器标志
    /// TIMER_ABSTIME: 将时间解释为绝对时间而非相对时间。
    pub const TIMER_ABSTIME: c_int = 0x01;
}
