// os/src/log/tests/mod.rs

use super::level::LogLevel;
use super::log_core::LogCore;
use crate::{kassert, test_case};

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
mod byte_counting;
mod filter;
mod format;
mod overflow;
mod peek;
