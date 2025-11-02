//! Logging configuration

/// Global log buffer size
pub const GLOBAL_LOG_BUFFER_SIZE: usize = 16 * 1024; // 16KB

/// Maximum length of a single log message
pub const MAX_LOG_MESSAGE_LENGTH: usize = 256;

/// Default log level
pub const DEFAULT_LOG_LEVEL: super::level::LogLevel = super::level::LogLevel::Info;
