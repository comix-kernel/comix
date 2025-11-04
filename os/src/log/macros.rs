//! Logging macros
//!
//! This module provides Linux kernel-style logging macros (`pr_*`) for easy logging
//! at different priority levels.
//!
//! # Usage
//!
//! ```rust
//! use crate::pr_info;
//! use crate::pr_err;
//!
//! pr_info!("System initialized");
//! pr_err!("Failed to allocate {} bytes", size);
//! pr_warn!("Memory usage at {}%", percent);
//! pr_debug!("Variable x = {}", x);
//! ```
//!
//! # Macro List
//!
//! - `pr_emerg!` - Emergency level (system unusable)
//! - `pr_alert!` - Alert level (immediate action required)
//! - `pr_crit!` - Critical level (critical conditions)
//! - `pr_err!` - Error level (error conditions)
//! - `pr_warn!` - Warning level (warning conditions)
//! - `pr_notice!` - Notice level (normal but significant)
//! - `pr_info!` - Info level (informational messages)
//! - `pr_debug!` - Debug level (debug messages)
//!
//! # Performance
//!
//! All macros check the global log level at macro expansion time. If a log level
//! is disabled, the format string is never evaluated, making disabled logs
//! essentially zero-cost.

/// Internal implementation macro with level filtering
///
/// Checks if the log level is enabled before calling the logging implementation.
/// This early check avoids unnecessary format string evaluation for disabled levels.
#[macro_export]
macro_rules! __log_impl_filtered {
    ($level:expr, $args:expr) => {
        if $crate::log::is_level_enabled($level) {
            $crate::log::log_impl($level, $args);
        }
    };
}

/// Logs a message at the EMERGENCY level
///
/// Emergency logs indicate the system is unusable. These are always printed
/// to console (if console output is available) and stored in the buffer.
///
/// # Examples
///
/// ```rust
/// pr_emerg!("Kernel panic: {}", reason);
/// pr_emerg!("System halt");
/// ```
#[macro_export]
macro_rules! pr_emerg {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Emergency,
            format_args!($($arg)*)
        )
    }
}

/// Logs a message at the ALERT level
///
/// Alert logs indicate action must be taken immediately.
///
/// # Examples
///
/// ```rust
/// pr_alert!("Critical hardware failure detected");
/// ```
#[macro_export]
macro_rules! pr_alert {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Alert,
            format_args!($($arg)*)
        )
    }
}

/// Logs a message at the CRITICAL level
///
/// Critical logs indicate critical conditions that need attention.
///
/// # Examples
///
/// ```rust
/// pr_crit!("Temperature threshold exceeded");
/// ```
#[macro_export]
macro_rules! pr_crit {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Critical,
            format_args!($($arg)*)
        )
    }
}

/// Logs a message at the ERROR level
///
/// Error logs indicate error conditions that occurred during operation.
///
/// # Examples
///
/// ```rust
/// pr_err!("Failed to allocate {} bytes", size);
/// pr_err!("Device initialization failed: {}", error);
/// ```
#[macro_export]
macro_rules! pr_err {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Error,
            format_args!($($arg)*)
        )
    }
}

/// Logs a message at the WARNING level
///
/// Warning logs indicate conditions that should be reviewed but don't prevent
/// normal operation.
///
/// # Examples
///
/// ```rust
/// pr_warn!("Memory usage at {}%", percent);
/// pr_warn!("Deprecated feature used");
/// ```
#[macro_export]
macro_rules! pr_warn {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Warning,
            format_args!($($arg)*)
        )
    }
}

/// Logs a message at the NOTICE level
///
/// Notice logs indicate normal but significant conditions.
///
/// # Examples
///
/// ```rust
/// pr_notice!("Device {} connected", device_name);
/// ```
#[macro_export]
macro_rules! pr_notice {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Notice,
            format_args!($($arg)*)
        )
    }
}

/// Logs a message at the INFO level
///
/// Info logs provide informational messages about normal system operation.
///
/// # Examples
///
/// ```rust
/// pr_info!("Kernel initialized");
/// pr_info!("Starting subsystem {}", name);
/// ```
#[macro_export]
macro_rules! pr_info {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Info,
            format_args!($($arg)*)
        )
    }
}

/// Logs a message at the DEBUG level
///
/// Debug logs provide detailed diagnostic information for troubleshooting.
/// These are typically disabled in production builds.
///
/// # Examples
///
/// ```rust
/// pr_debug!("Function called with x = {}", x);
/// pr_debug!("State transition: {} -> {}", old_state, new_state);
/// ```
#[macro_export]
macro_rules! pr_debug {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Debug,
            format_args!($($arg)*)
        )
    }
}
