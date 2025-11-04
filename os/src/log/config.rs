//! Logging system configuration constants
//!
//! This module defines compile-time configuration parameters for the logging system.

#![allow(unused)]

/// Total size of the global log buffer in bytes
///
/// The buffer is implemented as a fixed-size ring buffer. When full, new logs
/// overwrite the oldest entries. With a 16KB buffer and typical entry sizes,
/// this can store approximately 50-60 log entries.
pub(super) const GLOBAL_LOG_BUFFER_SIZE: usize = 16 * 1024; // 16KB

/// Maximum length of a single log message in bytes
///
/// Messages exceeding this length will be truncated. This limit prevents
/// individual logs from consuming excessive buffer space.
pub(super) const MAX_LOG_MESSAGE_LENGTH: usize = 256;

/// Default global log level
///
/// Logs at this level or higher priority will be recorded to the buffer.
/// Default is `Info`, meaning Debug logs are filtered out by default.
pub(super) const DEFAULT_LOG_LEVEL: super::level::LogLevel = super::level::LogLevel::Info;

/// Default console output level
///
/// Logs at this level or higher priority will be immediately printed to console.
/// Default is `Warning`, meaning only warnings and errors appear on console by default.
pub(super) const DEFAULT_CONSOLE_LEVEL: super::level::LogLevel = super::level::LogLevel::Warning;
