//! Lock-free ring buffer for log storage
//!
//! This module implements a high-performance, multi-producer single-consumer (MPSC)
//! ring buffer using atomic operations for synchronization.

use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicUsize, Ordering};

use super::config::GLOBAL_LOG_BUFFER_SIZE;
use super::entry::LogEntry;

/// Size of a single log entry in bytes
const LOG_ENTRY_SIZE: usize = core::mem::size_of::<LogEntry>();

/// Maximum number of log entries that can be stored in the buffer
pub(crate) const MAX_LOG_ENTRIES: usize = GLOBAL_LOG_BUFFER_SIZE / LOG_ENTRY_SIZE;

/// Cache-line padded wrapper to prevent false sharing
///
/// Pads the wrapped type to 64 bytes (typical cache line size) to ensure that
/// different atomic variables used by different CPU cores don't share cache lines,
/// which would cause performance degradation due to cache coherency traffic.
#[repr(C, align(64))]
struct CachePadded64<T> {
    inner: T,
}

impl<T> Deref for CachePadded64<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for CachePadded64<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// Global log buffer instance
///
/// We don't need a lock (like Mutex) or `Lazy` (like OnceCell) because:
///
/// 1.  **No `Mutex` (lock) is needed:** `GlobalLogBuffer` is inherently thread-safe.
///     It is designed with "interior mutability" using `AtomicUsize` fields.
///     Since all its methods (`write`, `read`) operate on a shared reference (`&self`)
///     and all internal mutation is handled safely by atomics, the entire
///     struct is `Sync`. We don't need to wrap an already thread-safe
///     type in *another* lock.
///
/// 2.  **No `Lazy` or `OnceCell` is needed:** The `GlobalLogBuffer::new()`
///     function is a `const fn`. This means it is executed at **compile-time**,
///     not at run-time. The entire, fully-initialized `GlobalLogBuffer`
///     instance is baked directly into the kernel's data segment (`.data` or `.bss`)
///     when the kernel is compiled.
///
///     There is no run-time initialization step, and therefore no "first-init"
///     race condition that `Lazy` is designed to solve. The buffer exists
///     in memory, fully initialized, from the very first CPU instruction.
///
/// This pattern results in a zero-cost, lock-free, and data-race-free
/// global static instance.
static GLOBAL_LOG_BUFFER: GlobalLogBuffer = GlobalLogBuffer::new();

/// Lock-free ring buffer for storing log entries
///
/// Uses a multi-producer single-consumer (MPSC) design where:
/// - Multiple CPUs can concurrently write logs without blocking
/// - A single consumer thread reads logs sequentially
#[repr(C)]
pub(super) struct GlobalLogBuffer {
    /// Writer-side data (updated by producers)
    writer_data: CachePadded64<WriterData>,
    /// Reader-side data (updated by consumer)
    reader_data: CachePadded64<ReaderData>,
    /// Fixed-size array of log entries
    buffer: [LogEntry; MAX_LOG_ENTRIES],
}

/// Writer-side synchronization data
#[repr(C)]
struct WriterData {
    /// Monotonically increasing sequence number for write operations
    write_seq: AtomicUsize,
}

/// Reader-side synchronization data
#[repr(C)]
struct ReaderData {
    /// Monotonically increasing sequence number for read operations
    read_seq: AtomicUsize,
    /// Count of logs dropped due to buffer overflow
    dropped: AtomicUsize,
}

impl GlobalLogBuffer {
    /// Creates a new global log buffer at compile time
    pub(super) const fn new() -> Self {
        const EMPTY: LogEntry = LogEntry::empty();
        Self {
            writer_data: CachePadded64 {
                inner: WriterData {
                    write_seq: AtomicUsize::new(1),
                },
            },
            reader_data: CachePadded64 {
                inner: ReaderData {
                    read_seq: AtomicUsize::new(1),
                    dropped: AtomicUsize::new(0),
                },
            },
            buffer: [EMPTY; MAX_LOG_ENTRIES],
        }
    }

    /// Writes a log entry to the buffer
    ///
    /// This is a lock-free operation that:
    /// 1. Atomically claims a sequence number (ticket)
    /// 2. Calculates the target slot using modulo arithmetic
    /// 3. Handles buffer overflow by advancing read pointer if needed
    /// 4. Copies log data to the slot
    /// 5. Publishes the entry with a Release memory barrier
    pub(super) fn write(&self, entry: &LogEntry) {
        // step1: Atomically claim a unique sequence number (ticket)
        let seq = self.writer_data.write_seq.fetch_add(1, Ordering::Relaxed);

        // step2: Calculate the target slot index from the sequence
        let slot = seq % MAX_LOG_ENTRIES;
        let slot_ptr = unsafe { self.buffer.as_ptr().add(slot) as *mut LogEntry };

        // step3: Check and handle potential buffer full (overwrite) logic
        self.handle_overwrite(seq);

        // step4: Copy all log data (*except* the seq field) to the slot
        unsafe {
            entry.copy_data_to(slot_ptr);
        }

        // step5: Publish the entry by atomically setting its seq (Release barrier)
        unsafe {
            entry.publish(slot_ptr, seq);
        }
    }

    /// Handles buffer overflow by advancing read pointer if needed
    ///
    /// When the buffer is full and a new write would overwrite unread entries,
    /// this function:
    /// 1. Detects the overflow condition
    /// 2. Calculates how many entries would be overwritten
    /// 3. Updates the dropped count
    /// 4. Atomically advances the read pointer using CAS loop
    fn handle_overwrite(&self, current_seq: usize) {
        let read_seq = self.reader_data.read_seq.load(Ordering::Acquire);
        if current_seq < read_seq + MAX_LOG_ENTRIES {
            return;
        }
        let new_read_seq = current_seq - MAX_LOG_ENTRIES + 1;
        let overwritten = new_read_seq.saturating_sub(read_seq);
        self.reader_data
            .dropped
            .fetch_add(overwritten, Ordering::Relaxed);

        // CAS loop to advance read_seq
        let mut current_read_seq = read_seq;
        while current_read_seq < new_read_seq {
            match self.reader_data.read_seq.compare_exchange_weak(
                current_read_seq,
                new_read_seq,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(seen_seq) => {
                    if seen_seq >= new_read_seq {
                        break;
                    }
                    current_read_seq = seen_seq;
                }
            }
        }
    }

    /// Reads the next log entry from the buffer
    ///
    /// Returns `None` if no entries are available. This is a lock-free
    /// single-consumer operation that uses Acquire memory ordering to ensure
    /// proper synchronization with producers.
    pub(super) fn read(&self) -> Option<LogEntry> {
        let read_seq = self.reader_data.read_seq.load(Ordering::Acquire);

        let slot = read_seq % MAX_LOG_ENTRIES;
        let slot_ptr = unsafe { self.buffer.as_ptr().add(slot) as *const LogEntry };

        const EMPTY: LogEntry = LogEntry::empty();
        unsafe {
            if !EMPTY.is_ready(slot_ptr, read_seq) {
                return None;
            }
        }

        let entry_data = unsafe { (*slot_ptr).clone() };

        self.reader_data
            .read_seq
            .store(read_seq + 1, Ordering::Release);

        Some(entry_data)
    }

    /// Returns the number of unread log entries in the buffer
    pub(super) fn len(&self) -> usize {
        let write = self.writer_data.write_seq.load(Ordering::Relaxed);
        let read = self.reader_data.read_seq.load(Ordering::Relaxed);
        write.saturating_sub(read)
    }

    /// Returns the total number of dropped logs due to buffer overflow
    pub(super) fn dropped_count(&self) -> usize {
        self.reader_data.dropped.load(Ordering::Relaxed)
    }
}

/// Writes a log entry to the global buffer (internal use)
#[inline]
pub(super) fn write_log(entry: &LogEntry) {
    GLOBAL_LOG_BUFFER.write(entry);
}

/// Reads the next log entry from the global buffer
///
/// Returns `None` if no entries are available for reading.
#[inline]
pub fn read_log() -> Option<LogEntry> {
    GLOBAL_LOG_BUFFER.read()
}

/// Returns the total count of logs dropped due to buffer overflow
#[inline]
pub fn log_dropped_count() -> usize {
    GLOBAL_LOG_BUFFER.dropped_count()
}

/// Returns the number of unread log entries currently in the buffer
#[inline]
pub fn log_len() -> usize {
    GLOBAL_LOG_BUFFER.len()
}
