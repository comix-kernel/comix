use crate::sbi::set_timer;
use riscv::register::time;
use crate::config::CLOCK_FREQ;

const TICKS_PER_SEC: usize = 100;
const MSEC_PER_SEC: usize = 1000;

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
    crate::arch::trap::enable_timer_interrupt();
}

mod tests {
    use super::*;
    #[test_case]
    fn test_set_next_trigger() {
        println!("Testing set_next_trigger...");
        let current_time = get_time();
        set_next_trigger();
        let next_time = get_time();
        assert!(next_time > current_time);
    }

    #[test_case]
    fn test_get_time_ms() {
        println!("Testing get_time_ms...");
        let time_ms = get_time_ms();
        assert!(time_ms > 0);
    }
}