// os/src/log/tests/mod.rs

use super::log_core::LogCore;
use super::level::LogLevel;

// ========== 测试辅助宏 ==========

/// 测试专用日志宏
///
/// 模拟生产宏的行为，但操作独立的 LogCore 实例
macro_rules! test_log {
    ($logger:expr, $level:expr, $($arg:tt)*) => {
        $logger._log($level, format_args!($($arg)*))
    };
}

// ========== 子模块 ==========
mod basic;
mod filter;
mod overflow;
mod format;
