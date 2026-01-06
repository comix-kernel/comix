//! LoongArch64 定时器模块

use core::sync::atomic::{AtomicUsize, Ordering};

use crate::{arch::intr::enable_timer_interrupt, earlyprintln};

/// 定时器滴答计数
pub static TIMER_TICKS: AtomicUsize = AtomicUsize::new(0);

/// 时钟频率 (Hz)
static CLOCK_FREQ: AtomicUsize = AtomicUsize::new(100_000_000); // 默认 100MHz，可在 init 中覆写

/// 每秒滴答数
pub const TICKS_PER_SEC: usize = 100;

// LoongArch 定时器相关 CSR 编号
const CSR_TCFG: u32 = 0x41;
const CSR_TICLR: u32 = 0x44;

/// 初始化定时器
pub fn init() {
    earlyprintln!("[Timer] Initializing timer");
    // 允许外部在平台层设置真实频率；此处仅开启本地定时器中断
    unsafe { enable_timer_interrupt() };
    earlyprintln!("[Timer] Timer interrupt enabled");
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
    let delta = (clock_freq() / TICKS_PER_SEC).max(1);

    unsafe {
        // 1. 先彻底关闭定时器并清除周期模式 (TCFG bit 0 and 1 = 0) 防止配置过程中的竞争
        core::arch::asm!("csrwr $r0, 0x410");

        // 2. 写入 TVAL (0x420)
        // 在 En=0 时写入 TVAL 会直接重置当前倒计时器
        core::arch::asm!(
            "csrwr {val}, 0x420",
            val = in(reg) delta
        );

        // 3. 开启定时器，设为单次触发模式 (En=1, Periodic=0)
        // 这样定时器倒数到 0 后会停下并触发中断
        let cfg = 0b01usize;
        core::arch::asm!(
            "csrwr {val}, 0x410",
            val = in(reg) cfg
        );
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
