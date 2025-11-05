//! Log level definitions
//!
//! This module defines the eight log levels used by the kernel logging system,
//! matching Linux kernel's `printk` levels.

/// Log level enumeration
///
/// Defines eight priority levels from Emergency (highest priority) to Debug (lowest).
/// The levels are compatible with Linux kernel's `KERN_*` constants.
///
/// # Level Semantics
///
/// - **Emergency**: System is unusable
/// - **Alert**: Action must be taken immediately
/// - **Critical**: Critical conditions
/// - **Error**: Error conditions
/// - **Warning**: Warning conditions
/// - **Notice**: Normal but significant condition
/// - **Info**: Informational messages
/// - **Debug**: Debug-level messages
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// System is unusable
    Emergency = 0,
    /// Action must be taken immediately
    Alert = 1,
    /// Critical conditions
    Critical = 2,
    /// Error conditions
    Error = 3,
    /// Warning conditions
    Warning = 4,
    /// Normal but significant condition
    Notice = 5,
    /// Informational messages
    Info = 6,
    /// Debug-level messages
    Debug = 7,
}

impl LogLevel {
    /// Returns the string representation of the log level
    ///
    /// Returns a short tag like `[ERR]`, `[INFO]`, etc.
    pub(super) const fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Emergency => "[EMERG]",
            LogLevel::Alert => "[ALERT]",
            LogLevel::Critical => "[CRIT]",
            LogLevel::Error => "[ERR]",
            LogLevel::Warning => "[WARNING]",
            LogLevel::Notice => "[NOTICE]",
            LogLevel::Info => "[INFO]",
            LogLevel::Debug => "[DEBUG]",
        }
    }

    /// Returns the ANSI color code for this log level
    ///
    /// # Color Mapping
    ///
    /// - Emergency/Alert/Critical: Bright red
    /// - Error: Red
    /// - Warning: Yellow
    /// - Notice: Bright white
    /// - Info: White
    /// - Debug: Gray
    pub(super) const fn color_code(&self) -> &'static str {
        match self {
            Self::Emergency | Self::Alert | Self::Critical => "\x1b[1;31m",
            Self::Error => "\x1b[31m",
            Self::Warning => "\x1b[33m",
            Self::Notice => "\x1b[1;37m",
            Self::Info => "\x1b[37m",
            Self::Debug => "\x1b[90m",
        }
    }

    /// Returns the ANSI color reset code
    pub(super) const fn reset_color_code(&self) -> &'static str {
        "\x1b[0m"
    }

    /// Converts a u8 value to a log level
    ///
    /// Returns the default log level if the value is invalid.
    pub(super) fn from_u8(level: u8) -> Self {
        match level {
            0 => Self::Emergency,
            1 => Self::Alert,
            2 => Self::Critical,
            3 => Self::Error,
            4 => Self::Warning,
            5 => Self::Notice,
            6 => Self::Info,
            7 => Self::Debug,

            _ => super::config::DEFAULT_LOG_LEVEL,
        }
    }
}
