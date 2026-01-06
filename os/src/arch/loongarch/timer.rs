//! LoongArch64 定时器模块

use core::sync::atomic::{AtomicUsize, Ordering};

use loongArch64::register::{tcfg, ticlr};
use loongArch64::time::{Time, get_timer_freq};

use crate::kernel::CLOCK_FREQ;

/// 定时器滴答计数
pub static TIMER_TICKS: AtomicUsize = AtomicUsize::new(0);

/// 每秒滴答数
pub const TICKS_PER_SEC: usize = 100;
/// 每秒毫秒数
pub const MSEC_PER_SEC: usize = 1000;

/// 初始化定时器
pub fn init() {
    let freq = get_timer_freq();
    // SAFETY: 仅在初始化阶段设置时钟频率
    unsafe { CLOCK_FREQ = freq };
    set_next_trigger();
    // SAFETY: 初始化阶段配置 CSR 定时器中断
    unsafe { crate::arch::intr::enable_timer_interrupt() };
}

/// 获取当前时间（硬件计数器值）
#[inline]
pub fn get_time() -> usize {
    Time::read()
}

/// 获取当前滴答数
#[inline]
pub fn get_ticks() -> usize {
    TIMER_TICKS.load(Ordering::Relaxed)
}

/// 获取时钟频率
#[inline]
pub fn clock_freq() -> usize {
    // SAFETY: CLOCK_FREQ 在初始化阶段写入
    unsafe { CLOCK_FREQ }
}

/// 设置下一次定时器中断
pub fn set_next_trigger() {
    let mut interval = clock_freq() / TICKS_PER_SEC;
    if interval < 4 {
        interval = 4;
    }
    interval = (interval + 3) & !3;
    tcfg::set_init_val(interval);
    tcfg::set_periodic(true);
    tcfg::set_en(true);
    ticlr::clear_timer_interrupt();
}

/// 获取当前时间（毫秒）
#[inline]
pub fn get_time_ms() -> usize {
    (get_time() as u128 * MSEC_PER_SEC as u128 / clock_freq() as u128) as usize
}
