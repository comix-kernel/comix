use core::sync::atomic::{AtomicU8, Ordering};

use super::config::{DEFAULT_CONSOLE_LEVEL, DEFAULT_LOG_LEVEL};
use super::level::LogLevel;

static GLOBAL_LOG_LEVEL: AtomicU8 = AtomicU8::new(DEFAULT_LOG_LEVEL as u8);
static CONSOLE_LEVEL: AtomicU8 = AtomicU8::new(DEFAULT_CONSOLE_LEVEL as u8);

#[inline(always)]
pub fn is_level_enabled(level: LogLevel) -> bool {
    let global_level = GLOBAL_LOG_LEVEL.load(Ordering::Relaxed);
    level as u8 <= global_level
}

pub fn set_global_level(level: LogLevel) {
    GLOBAL_LOG_LEVEL.store(level as u8, Ordering::Release);
}

pub fn get_global_level() -> LogLevel {
    let level = GLOBAL_LOG_LEVEL.load(Ordering::Relaxed);
    LogLevel::from_u8(level)
}

#[inline(always)]
pub fn is_console_level(level: LogLevel) -> bool {
    let console_level = CONSOLE_LEVEL.load(Ordering::Relaxed);
    level as u8 <= console_level
}

pub fn set_console_level(level: LogLevel) {
    CONSOLE_LEVEL.store(level as u8, Ordering::Release);
}

pub fn get_console_level() -> LogLevel {
    let level = CONSOLE_LEVEL.load(Ordering::Relaxed);
    LogLevel::from_u8(level)
}
