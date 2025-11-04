//! Two-tier log level filtering
//!
//! This module implements a dual-level filtering system:
//!
//! 1. **Global Level**: Controls which logs are recorded to the buffer
//! 2. **Console Level**: Controls which logs are immediately printed to console
//!
//! Both levels can be dynamically adjusted at runtime.

use core::sync::atomic::{AtomicU8, Ordering};

use super::config::{DEFAULT_CONSOLE_LEVEL, DEFAULT_LOG_LEVEL};
use super::level::LogLevel;

/// Global log level threshold
///
/// Logs with level <= this threshold are recorded to the buffer.
static GLOBAL_LOG_LEVEL: AtomicU8 = AtomicU8::new(DEFAULT_LOG_LEVEL as u8);

/// Console output level threshold
///
/// Logs with level <= this threshold are immediately printed to console.
static CONSOLE_LEVEL: AtomicU8 = AtomicU8::new(DEFAULT_CONSOLE_LEVEL as u8);

/// Checks if a log level is enabled for recording (first filter)
///
/// This is called at macro expansion time to avoid unnecessary work
/// for disabled log levels.
#[inline(always)]
#[doc(hidden)]
pub fn is_level_enabled(level: LogLevel) -> bool {
    let global_level = GLOBAL_LOG_LEVEL.load(Ordering::Relaxed);
    level as u8 <= global_level
}

/// Sets the global log level threshold
///
/// Logs with priority higher than this level will be discarded.
pub fn set_global_level(level: LogLevel) {
    GLOBAL_LOG_LEVEL.store(level as u8, Ordering::Release);
}

/// Gets the current global log level
pub fn get_global_level() -> LogLevel {
    let level = GLOBAL_LOG_LEVEL.load(Ordering::Relaxed);
    LogLevel::from_u8(level)
}

/// Checks if a log level should be printed to console (second filter)
#[inline(always)]
pub(super) fn is_console_level(level: LogLevel) -> bool {
    let console_level = CONSOLE_LEVEL.load(Ordering::Relaxed);
    level as u8 <= console_level
}

/// Sets the console output level threshold
///
/// Only logs with priority equal to or higher than this level will be
/// immediately printed to the console.
pub fn set_console_level(level: LogLevel) {
    CONSOLE_LEVEL.store(level as u8, Ordering::Release);
}

/// Gets the current console output level
pub fn get_console_level() -> LogLevel {
    let level = CONSOLE_LEVEL.load(Ordering::Relaxed);
    LogLevel::from_u8(level)
}
