//! Log context collection
//!
//! This module collects contextual information (CPU ID, task ID, timestamp)
//! for each log entry.

use crate::arch::timer;

/// Contextual information for a log entry
pub(super) struct LogContext {
    /// ID of the CPU that generated the log
    pub(super) cpu_id: usize,
    /// ID of the task/process that generated the log
    pub(super) task_id: u32,
    /// Timestamp when the log was created
    pub(super) timestamp: usize,
}

/// Collects context information for a new log entry
///
/// # Implementation Status
///
/// - **Timestamp**: Implemented via `arch::timer::get_time()`
/// - **CPU ID**: TODO - needs multi-core support implementation
/// - **Task ID**: TODO - needs task management integration
pub(super) fn collect_context() -> LogContext {
    LogContext {
        cpu_id: 0,  // TODO: replace with current cpu id
        task_id: 0, // TODO: replace with current task id
        timestamp: timer::get_time(),
    }
}
