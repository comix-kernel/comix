// os/src/log/tests/mod.rs

use super::log_core::LogCore;
use super::level::LogLevel;

// ========== Test Helper Macros ==========

/// Test-specific logging macro
///
/// Simulates production macro behavior but operates on an independent LogCore instance
macro_rules! test_log {
    ($logger:expr, $level:expr, $($arg:tt)*) => {
        $logger._log($level, format_args!($($arg)*))
    };
}

// ========== Sub-modules ==========
mod basic;
mod filter;
mod overflow;
mod format;
