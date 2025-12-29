//! LoongArch64 定时器模块

use core::sync::atomic::{AtomicUsize, Ordering};

use crate::arch::intr::enable_timer_interrupt;

/// 定时器滴答计数
pub static TIMER_TICKS: AtomicUsize = AtomicUsize::new(0);

/// 时钟频率 (Hz)
static CLOCK_FREQ: AtomicUsize = AtomicUsize::new(100_000_000); // 默认 100MHz，可在 init 中覆写

/// 每秒滴答数
pub const TICKS_PER_SEC: usize = 100;

// LoongArch 定时器相关 CSR 编号
const CSR_TCFG: u32 = 0x41;
const CSR_TVAL: u32 = 0x42;
const CSR_TICLR: u32 = 0x44;

/// 初始化定时器
pub fn init() {
    // 允许外部在平台层设置真实频率；此处仅开启本地定时器中断
    unsafe { enable_timer_interrupt() };
    set_next_trigger();
}

/// 读取当前时间（硬件计数器）
pub fn get_time() -> usize {
    let time: usize;
    unsafe {
        // LoongArch rdtime.d 需要显式提供第二操作数（通常为 $zero）。
        core::arch::asm!("rdtime.d {time}, $zero", time = out(reg) time, options(nostack, preserves_flags));
    }
    time
}

/// 获取当前滴答数（软件累计）
pub fn get_ticks() -> usize {
    TIMER_TICKS.load(Ordering::Relaxed)
}

/// 获取时钟频率
pub fn clock_freq() -> usize {
    CLOCK_FREQ.load(Ordering::Relaxed)
}

/// 设置下一次定时器中断
pub fn set_next_trigger() {
    // 目标间隔 = 1 / TICKS_PER_SEC 秒
    let delta = (clock_freq() / TICKS_PER_SEC).max(1);

    unsafe {
        // 写入 TVAL 作为下一次计数起点
        core::arch::asm!("csrwr {val}, {tval}", val = in(reg) delta, tval = const CSR_TVAL, options(nostack, preserves_flags));

        // TCFG: [0]=EN, [1]=PERIODIC, [63:2]=初始计数值（这里使用 delta）
        let cfg = ((delta as u64) << 2) | 0b11;
        core::arch::asm!("csrwr {val}, {tcfg}", val = in(reg) cfg, tcfg = const CSR_TCFG, options(nostack, preserves_flags));
    }
}

/// 确认/清除定时器中断挂起位
pub fn ack_timer_interrupt() {
    unsafe {
        core::arch::asm!("csrwr {val}, {ticlr}", val = in(reg) 1usize, ticlr = const CSR_TICLR, options(nostack, preserves_flags));
    }
}

/// 获取当前时间（毫秒）
pub fn get_time_ms() -> usize {
    get_ticks() * 1000 / TICKS_PER_SEC
}
