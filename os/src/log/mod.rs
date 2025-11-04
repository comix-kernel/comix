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
//! - [`entry`]: Log entry structure and serialization
//! - [`filter`]: Two-tier log level filtering
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
mod entry;
mod filter;
mod level;
pub mod macros;

pub use entry::LogEntry;
pub use level::LogLevel;

// Re-export public APIs for reading logs
pub use buffer::{log_dropped_count, log_len, read_log};

// Re-export public APIs for configuring log levels
pub use filter::{get_console_level, get_global_level, set_console_level, set_global_level};

// Re-export for internal use by macros (hidden from docs)
#[doc(hidden)]
pub use filter::is_level_enabled;

/// Core logging implementation called by all logging macros
///
/// This function:
/// 1. Collects context information (CPU ID, task ID, timestamp)
/// 2. Creates a log entry from the format arguments
/// 3. Immediately prints to console if the log meets console level threshold
/// 4. Writes the log to the ring buffer for later consumption
///
/// # Parameters
///
/// * `level` - The log level (Emergency, Error, Info, etc.)
/// * `args` - Formatted arguments from `format_args!` macro
#[doc(hidden)]
pub fn log_impl(level: LogLevel, args: core::fmt::Arguments) {
    // Collect context information
    let log_context = context::collect_context();
    let (cpu_id, task_id, timestamp) = (
        log_context.cpu_id,
        log_context.task_id,
        log_context.timestamp,
    );

    // Create log entry
    let entry = LogEntry::from_args(level, cpu_id, task_id, timestamp, args);

    // Immediate console output for urgent logs
    if filter::is_console_level(level) {
        direct_print_entry(&entry);
    }

    // Always buffer the log for later retrieval
    buffer::write_log(&entry);
}

/// Prints a log entry directly to the console with ANSI color codes
fn direct_print_entry(entry: &LogEntry) {
    // Important!: must lock console here to prevent:
    // garbled (interleaved) output from concurrent calls.
    //
    // This function is the single "choke point" for all physical
    // console I/O and can be called concurrently from 2 different sources:
    //
    // 1. **Urgent Logs:** Multiple CPUs hitting high-priority logs (e.g., `pr_err!`).
    // 2. **Async Consumer:** The `console_flush_thread` printing buffered logs.
    //
    // A global `CONSOLE_LOCK` (SpinLock) must be acquired before these
    // `write!` operations to serialize all access to the (e.g.) UART hardware.
    //
    // let _guard = CONSOLE_LOCK.lock(); // <-- lock here

    use crate::console::Stdout;
    use core::fmt::Write;

    let mut stdout = Stdout;
    let _ = write!(
        stdout,
        "{}{} ",
        entry.level().color_code(),
        entry.level().as_str()
    );
    let _ = stdout.write_str(entry.message());
    let _ = write!(stdout, "{}", entry.level().reset_color_code());
    let _ = writeln!(stdout);
}
