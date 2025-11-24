#![allow(non_camel_case_types)]
#![allow(dead_code)]

use core::ffi::{c_int, c_long};

/// 用于指定秒和纳秒精度的时间间隔。
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct timespec {
    /// 秒 (seconds)
    pub tv_sec: c_long,
    /// 纳秒 (nanoseconds)
    pub tv_nsec: c_long,
}

/// 用于指定秒和微秒精度的时间间隔。
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct timeval {
    /// 秒 (seconds)
    pub tv_sec: c_long,
    /// 微秒 (microseconds)
    pub tv_usec: c_long,
}

/// 用于设置 POSIX 间隔定时器 (timer_create) 的结构。
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct itimerspec {
    /// 定时器周期 (timer period)
    pub it_interval: timespec,
    /// 定时器初始值/到期时间 (timer expiration)
    pub it_value: timespec,
}

/// 用于设置传统 BSD 间隔定时器 (setitimer) 的结构。
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct itimerval {
    /// 定时器周期 (timer interval)
    pub it_interval: timeval,
    /// 定时器当前值 (current value)
    pub it_value: timeval,
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
