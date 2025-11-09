//! RISC-V 架构的定时器实现
//!
//! 包含定时器初始化、时间获取和定时器中断设置等功能
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::{arch::lib::sbi::set_timer, config::CLOCK_FREQ};
use riscv::register::time;

const TICKS_PER_SEC: usize = 100;
#[allow(dead_code)]
const MSEC_PER_SEC: usize = 1000;

// 记录时钟中断次数
pub static TIMER_TICKS: AtomicUsize = AtomicUsize::new(0);

// 获取当前tick数的
pub fn get_ticks() -> usize {
    TIMER_TICKS.load(Ordering::Relaxed)
}

/// 获取当前时间（以 ticks 为单位）
pub fn get_time() -> usize {
    time::read()
}

/// 获取当前时间（以毫秒为单位）
pub fn get_time_ms() -> usize {
    time::read() * MSEC_PER_SEC / CLOCK_FREQ
}

/// 设置定时器中断
pub fn set_next_trigger() {
    let next = get_time() + CLOCK_FREQ / TICKS_PER_SEC;
    set_timer(next);
}

/// 初始化定时器
pub fn init() {
    set_next_trigger();
    // Safe: 只在内核初始化阶段调用，确保唯一性
    unsafe { crate::arch::intr::enable_timer_interrupt() };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kassert, println, test_case};
    test_case!(test_set_next_trigger, {
        let current_time = get_time();
        set_next_trigger();
        let next_time = get_time();
        kassert!(next_time > current_time);
    });

    // test_case!(test_timer_ticks_increment, {
    //     crate::arch::trap::init_boot_trap();
    //     unsafe {
    //         crate::arch::intr::enable_interrupts();
    //         crate::arch::intr::enable_timer_interrupt();
    //     }
    //     let initial_ticks = TIMER_TICKS.load(Ordering::Relaxed);
    //     // 模拟等待一段时间以触发定时器中断
    //     let mut i = 0;

    //     while i < 1000000 {
    //         core::hint::spin_loop();
    //         i += 1;
    //     }
    //     let later_ticks = TIMER_TICKS.load(Ordering::Relaxed);
    //     kassert!(later_ticks > initial_ticks);
    //     unsafe {
    //         crate::arch::intr::disable_timer_interrupt();
    //         crate::arch::intr::disable_interrupts();
    //     }
    // });

    test_case!(test_get_time, {
        println!("Testing get_time...");
        let time = get_time();
        kassert!(time > 0);
    });

    test_case!(test_get_time_ms, {
        println!("Testing get_time_ms...");
        let time_ms = get_time_ms();
        kassert!(time_ms > 0);
    });
}
