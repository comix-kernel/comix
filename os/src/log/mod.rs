//! Kernel logging subsystem
//!
//! This module provides a Linux kernel-style logging system with lock-free
//! ring buffer implementation for bare-metal environments.
//!
//! # Components
//!
//! - [`buffer`]: Lock-free ring buffer for log storage
//! - [`config`]: Configuration constants (buffer size, message length limits)
//! - [`context`]: Context collection (CPU ID, task ID, timestamp)
//! - [`core`]: Core logging implementation (LogCore)
//! - [`entry`]: Log entry structure and serialization
//! - [`level`]: Log level definitions (Emergency to Debug)
//! - [`macros`]: User-facing logging macros (`pr_info!`, `pr_err!`, etc.)
//!
//! # Design Overview
//!
//! ## Dual-Output Strategy
//!
//! The logging system employs a two-tier approach:
//!
//! 1. **Immediate Console Output**: Logs meeting the console level threshold
//!    (default: Warning and above) are printed directly to the console for
//!    urgent visibility.
//! 2. **Ring Buffer Storage**: All logs meeting the global level threshold
//!    (default: Info and above) are written to a lock-free ring buffer for
//!    asynchronous consumption or post-mortem analysis.
//!
//! ## Performance Characteristics
//!
//! - **Lock-Free Concurrency**: Uses atomic operations (fetch_add, CAS) instead
//!   of mutexes, enabling multi-producer logging without blocking.
//! - **Early Filtering**: Log level checks occur at macro expansion time,
//!   avoiding format string evaluation for disabled levels.
//! - **Fixed-Size Allocation**: No dynamic memory allocation; all structures
//!   use compile-time-known sizes suitable for bare-metal environments.
//! - **Cache Optimization**: Reader/writer data structures are cache-line
//!   padded (64 bytes) to prevent false sharing on multi-core systems.
//! - **Zero-Copy Where Possible**: Log entries are constructed in-place when
//!   feasible to minimize memory operations.
//!
//! ## Architecture-Specific Integration
//!
//! The logging system integrates with architecture-specific components:
//!
//! - **Timer**: Timestamp collection via `arch::timer::get_time()`
//! - **Console**: Output via `console::Stdout` (typically UART)
//! - **CPU ID**: Multi-core support (TODO: implement `arch::cpu::current_cpu_id()`)
//! - **Task ID**: Task tracking (TODO: implement task management integration)
//!
//! # Usage Example
//!
//! ```rust
//! use crate::log::*;
//!
//! // Basic logging
//! pr_info!("Kernel initialized");
//! pr_err!("Failed to allocate {} bytes", size);
//!
//! // Configure log levels
//! set_global_level(LogLevel::Debug);  // Record all levels
//! set_console_level(LogLevel::Error); // Only print errors and above
//!
//! // Read buffered logs
//! while let Some(entry) = read_log() {
//!     // Process log entry
//! }
//! ```

mod buffer;
mod config;
mod context;
mod log_core;
mod entry;
mod level;
pub mod macros;

pub use entry::LogEntry;
pub use level::LogLevel;

// ========== Global Singleton ==========

/// Global logging system instance
///
/// Initialized at compile time using const fn, zero runtime overhead.
/// All logging macros and public APIs delegate to this instance.
static GLOBAL_LOG: log_core::LogCore = log_core::LogCore::default();

// ========== Public API (thin wrappers) ==========

/// Core logging implementation (called by macros)
#[doc(hidden)]
pub fn log_impl(level: LogLevel, args: core::fmt::Arguments) {
    GLOBAL_LOG._log(level, args);
}

/// Checks if a log level is enabled (called by macros)
#[doc(hidden)]
pub fn is_level_enabled(level: LogLevel) -> bool {
    level as u8 <= GLOBAL_LOG._get_global_level() as u8
}

/// Reads the next log entry from the buffer
pub fn read_log() -> Option<LogEntry> {
    GLOBAL_LOG._read_log()
}

/// Returns the number of unread log entries
pub fn log_len() -> usize {
    GLOBAL_LOG._log_len()
}

/// Returns the count of dropped logs
pub fn log_dropped_count() -> usize {
    GLOBAL_LOG._log_dropped_count()
}

/// Sets the global log level threshold
pub fn set_global_level(level: LogLevel) {
    GLOBAL_LOG._set_global_level(level);
}

/// Gets the current global log level
pub fn get_global_level() -> LogLevel {
    GLOBAL_LOG._get_global_level()
}

/// Sets the console output level threshold
pub fn set_console_level(level: LogLevel) {
    GLOBAL_LOG._set_console_level(level);
}

/// Gets the current console output level
pub fn get_console_level() -> LogLevel {
    GLOBAL_LOG._get_console_level()
}

// ========== Test module ==========
#[cfg(test)]
mod tests;
