use core::sync::atomic::AtomicUsize;

pub const TICKS_PER_SEC: usize = 100;
pub const MSEC_PER_SEC: usize = 1000;
pub static TIMER_TICKS: AtomicUsize = AtomicUsize::new(0);

pub fn get_ticks() -> usize {
    TIMER_TICKS.load(core::sync::atomic::Ordering::Relaxed)
}

pub fn get_time() -> usize {
    0
}

pub fn get_time_ms() -> usize {
    0
}

pub fn clock_freq() -> usize {
    10_000_000
}

pub fn set_next_trigger() {}

pub fn init() {}
