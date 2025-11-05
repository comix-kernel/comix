//! Log system core implementation
//!
//! This module encapsulates all logging state and logic into a single
//! `LogCore` struct that can be instantiated independently for testing
//! while maintaining the same lock-free, zero-allocation design.

use super::buffer::GlobalLogBuffer;
use super::config::{DEFAULT_CONSOLE_LEVEL, DEFAULT_LOG_LEVEL};
use super::context;
use super::entry::LogEntry;
use super::level::LogLevel;
use core::fmt;
use core::sync::atomic::{AtomicU8, Ordering};

/// Core logging system
///
/// Encapsulates the ring buffer and filtering state. Can be instantiated
/// for testing or used as a global singleton in production.
///
/// # Thread Safety
///
/// All methods use atomic operations for synchronization, making the entire
/// struct safe to share across threads without external locking.
pub struct LogCore {
    /// Lock-free ring buffer for log storage
    buffer: GlobalLogBuffer,

    /// Global log level threshold (controls buffering)
    global_level: AtomicU8,

    /// Console output level threshold (controls immediate printing)
    console_level: AtomicU8,
}

impl LogCore {
    /// Creates a new LogCore instance with default log levels
    ///
    /// This is a `const fn` that can be evaluated at compile time,
    /// allowing for zero-cost static initialization.
    ///
    /// Uses default levels from config:
    /// - Global level: Info (logs Debug are filtered)
    /// - Console level: Warning (only warnings and errors printed)
    ///
    /// # Examples
    ///
    /// ```rust
    /// // Global singleton (compile-time init)
    /// static GLOBAL_LOG: LogCore = LogCore::default();
    /// ```
    pub const fn default() -> Self {
        Self {
            buffer: GlobalLogBuffer::new(),
            global_level: AtomicU8::new(DEFAULT_LOG_LEVEL as u8),
            console_level: AtomicU8::new(DEFAULT_CONSOLE_LEVEL as u8),
        }
    }

    /// Creates a new LogCore instance with custom log levels
    ///
    /// This constructor allows specifying both global and console log levels
    /// at creation time, which is particularly useful for testing.
    ///
    /// # Parameters
    ///
    /// * `global_level` - Minimum level for logs to be buffered
    /// * `console_level` - Minimum level for logs to be printed to console
    ///
    /// # Examples
    ///
    /// ```rust
    /// // Test instance with Debug level enabled
    /// let test_log = LogCore::new(LogLevel::Debug, LogLevel::Warning);
    ///
    /// // Production instance with custom levels
    /// let log = LogCore::new(LogLevel::Info, LogLevel::Error);
    /// ```
    pub fn new(global_level: LogLevel, console_level: LogLevel) -> Self {
        Self {
            buffer: GlobalLogBuffer::new(),
            global_level: AtomicU8::new(global_level as u8),
            console_level: AtomicU8::new(console_level as u8),
        }
    }

    /// Core logging implementation
    ///
    /// This method is called by both production macros (via GLOBAL_LOG)
    /// and test code (via local instances).
    ///
    /// # Lock-Free Operation
    ///
    /// 1. Atomic read of global_level (Acquire)
    /// 2. Early return if filtered
    /// 3. Collect context (timestamp, CPU ID, task ID)
    /// 4. Create log entry (stack allocation)
    /// 5. Atomic buffer write (lock-free)
    /// 6. Optional console output (if meets console_level)
    ///
    /// # Parameters
    ///
    /// * `level` - Log level (Emergency to Debug)
    /// * `args` - Format arguments from `format_args!`
    pub fn _log(&self, level: LogLevel, args: fmt::Arguments) {
        // 1. Early filtering (global level)
        if !self.is_level_enabled(level) {
            return;
        }

        // 2. Collect context
        let log_context = context::collect_context();

        // 3. Create log entry
        let entry = LogEntry::from_args(
            level,
            log_context.cpu_id,
            log_context.task_id,
            log_context.timestamp,
            args,
        );

        // 4. Write to buffer (lock-free)
        self.buffer.write(&entry);

        // 5. Optional immediate console output
        if self.is_console_level(level) {
            self.direct_print_entry(&entry);
        }
    }

    /// Reads the next log entry from the buffer
    ///
    /// Returns `None` if no entries are available. This is a lock-free
    /// single-consumer operation.
    pub fn _read_log(&self) -> Option<LogEntry> {
        self.buffer.read()
    }

    /// Returns the number of unread log entries
    pub fn _log_len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns the count of logs dropped due to buffer overflow
    pub fn _log_dropped_count(&self) -> usize {
        self.buffer.dropped_count()
    }

    /// Sets the global log level threshold
    ///
    /// Logs with level > threshold will be discarded.
    ///
    /// # Memory Ordering
    ///
    /// Uses Release ordering to ensure the new level is visible to all cores.
    pub fn _set_global_level(&self, level: LogLevel) {
        self.global_level.store(level as u8, Ordering::Release);
    }

    /// Gets the current global log level
    pub fn _get_global_level(&self) -> LogLevel {
        let level = self.global_level.load(Ordering::Acquire);
        LogLevel::from_u8(level)
    }

    /// Sets the console output level threshold
    ///
    /// Only logs with level <= threshold will be immediately printed.
    pub fn _set_console_level(&self, level: LogLevel) {
        self.console_level.store(level as u8, Ordering::Release);
    }

    /// Gets the current console output level
    pub fn _get_console_level(&self) -> LogLevel {
        let level = self.console_level.load(Ordering::Acquire);
        LogLevel::from_u8(level)
    }

    // ========== Internal helpers ==========

    /// Checks if a log level is enabled (global filter)
    #[inline(always)]
    fn is_level_enabled(&self, level: LogLevel) -> bool {
        level as u8 <= self.global_level.load(Ordering::Acquire)
    }

    /// Checks if a log should be printed to console
    #[inline(always)]
    fn is_console_level(&self, level: LogLevel) -> bool {
        level as u8 <= self.console_level.load(Ordering::Acquire)
    }

    /// Prints a log entry directly to the console with ANSI colors
    fn direct_print_entry(&self, entry: &LogEntry) {
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
}

// Mark as Sync to allow use in static
unsafe impl Sync for LogCore {}