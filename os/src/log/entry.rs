//! Log entry structure and serialization
//!
//! This module defines the `LogEntry` structure that represents a single log message
//! along with its metadata, and provides utilities for creating and formatting log entries.

use super::config::MAX_LOG_MESSAGE_LENGTH;
use super::level::LogLevel;
use core::cmp::min;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicUsize, Ordering};

/// A single log entry with metadata and message
///
/// The structure is carefully laid out for lock-free synchronization:
/// - The `seq` field is used as a synchronization point between producers and consumers
/// - C representation with 8-byte alignment ensures proper atomic access
/// - Fields are ordered to minimize padding
#[repr(C, align(8))]
#[derive(Debug)]
pub struct LogEntry {
    /// Sequence number for synchronization (must be first field)
    seq: AtomicUsize,
    /// Log level (Emergency, Error, Info, etc.)
    level: LogLevel,
    /// CPU ID that generated this log
    cpu_id: usize,
    /// Actual length of the message in bytes
    length: usize,
    /// Task/process ID that generated this log
    task_id: u32,
    /// Timestamp when the log was created
    timestamp: usize,
    /// Fixed-size buffer for the log message
    message: [u8; MAX_LOG_MESSAGE_LENGTH],
}

impl LogEntry {
    /// Creates an empty log entry (used for initialization)
    ///
    /// This is a `const fn` so it can be evaluated at compile time,
    /// allowing for const initialization of the global buffer.
    pub const fn empty() -> Self {
        Self {
            seq: AtomicUsize::new(0),
            level: LogLevel::Debug,
            cpu_id: 0,
            length: 0,
            task_id: 0,
            timestamp: 0,
            message: [0; MAX_LOG_MESSAGE_LENGTH],
        }
    }

    /// Creates a log entry from format arguments
    ///
    /// # Parameters
    ///
    /// * `level` - Log level
    /// * `cpu_id` - ID of the CPU generating the log
    /// * `task_id` - ID of the task generating the log
    /// * `timestamp` - Timestamp of the log
    /// * `args` - Formatted arguments from `format_args!` macro
    pub(super) fn from_args(
        level: LogLevel,
        cpu_id: usize,
        task_id: u32,
        timestamp: usize,
        args: fmt::Arguments,
    ) -> Self {
        let mut entry = Self {
            seq: AtomicUsize::new(0),
            level,
            cpu_id,
            length: 0,
            task_id,
            timestamp,
            message: [0; MAX_LOG_MESSAGE_LENGTH],
        };

        // Format the message into the fixed-size buffer
        let mut writer = MessageWriter::new(&mut entry.message);
        let _ = core::fmt::write(&mut writer, args);

        entry.length = writer.len();

        entry
    }

    /// Returns the log message as a string slice
    pub fn message(&self) -> &str {
        // Safety: MessageWriter ensures valid UTF-8
        unsafe { core::str::from_utf8_unchecked(&self.message[..self.length]) }
    }

    /// Returns the log level
    pub fn level(&self) -> LogLevel {
        self.level
    }

    /// Returns the CPU ID that generated this log
    pub fn cpu_id(&self) -> usize {
        self.cpu_id
    }

    /// Returns the task ID that generated this log
    pub fn task_id(&self) -> u32 {
        self.task_id
    }

    /// Returns the timestamp of this log
    pub fn timestamp(&self) -> usize {
        self.timestamp
    }
}

impl LogEntry {
    /// Copies log data to a buffer slot (for internal use)
    ///
    /// Copies all fields except the `seq` field, which must be set separately
    /// via `publish()` to ensure proper memory ordering.
    ///
    /// # Safety
    ///
    /// `dest` must point to a valid `LogEntry` in the ring buffer
    pub(super) unsafe fn copy_data_to(&self, dest: *mut LogEntry) {
        // We can't use ptr::write because it would overwrite dest.seq
        // We must copy field by field, *except* seq
        (*dest).level = self.level;
        (*dest).cpu_id = self.cpu_id;
        (*dest).length = self.length;
        (*dest).task_id = self.task_id;
        (*dest).timestamp = self.timestamp;
        (*dest).message.copy_from_slice(&self.message);
    }

    /// Publishes the entry by setting its sequence number (for internal use)
    ///
    /// Uses Release memory ordering to ensure all data writes are visible
    /// to other cores before the sequence number update.
    ///
    /// # Safety
    ///
    /// `dest` must point to a valid `LogEntry` in the ring buffer
    pub(super) unsafe fn publish(&self, dest: *mut LogEntry, seq_num: usize) {
        // Use Release memory ordering to ensure all data writes
        // are visible before the 'seq' update
        (*dest).seq.store(seq_num, Ordering::Release);
    }

    /// Checks if a slot is ready for reading (for internal use)
    ///
    /// Uses Acquire memory ordering to pair with the producer's Release store
    /// in `publish()`, ensuring proper synchronization.
    ///
    /// # Safety
    ///
    /// `slot_ptr` must point to a valid `LogEntry` in the ring buffer
    pub(super) unsafe fn is_ready(&self, slot_ptr: *const LogEntry, expected_seq: usize) -> bool {
        // Use Acquire memory ordering to pair with producer's Release store
        (*slot_ptr).seq.load(Ordering::Acquire) == expected_seq
    }
}

impl Clone for LogEntry {
    fn clone(&self) -> Self {
        Self {
            seq: AtomicUsize::new(self.seq.load(Ordering::Relaxed)),
            level: self.level,
            cpu_id: self.cpu_id,
            length: self.length,
            task_id: self.task_id,
            timestamp: self.timestamp,
            message: self.message,
        }
    }
}

impl fmt::Display for LogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:12}] [{}] [CPU{}/T{:3}] {}",
            self.timestamp,
            self.level.as_str(),
            self.cpu_id,
            self.task_id,
            self.message()
        )
    }
}

/// Helper struct to write formatted output to a fixed-size byte buffer
///
/// Implements `core::fmt::Write` to capture formatted output from `format_args!`
/// without dynamic allocation. Messages exceeding the buffer size are truncated.
struct MessageWriter<'a> {
    buffer: &'a mut [u8],
    pos: usize,
}

impl<'a> MessageWriter<'a> {
    /// Creates a new message writer with the given buffer
    fn new(buffer: &'a mut [u8]) -> Self {
        Self { buffer, pos: 0 }
    }

    /// Returns the number of bytes written so far
    fn len(&self) -> usize {
        self.pos
    }
}

impl<'a> Write for MessageWriter<'a> {
    /// Writes a string slice to the buffer, truncating if necessary
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let remaining = self.buffer.get_mut(self.pos..).unwrap_or(&mut []);
        let to_copy = min(bytes.len(), remaining.len());

        remaining[..to_copy].copy_from_slice(&bytes[..to_copy]);
        self.pos += to_copy;
        Ok(())
    }
}
