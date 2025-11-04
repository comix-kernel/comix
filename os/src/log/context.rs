use crate::arch::timer;

pub(super) struct LogContext {
    pub(super) cpu_id: usize,
    pub(super) task_id: u32,
    pub(super) timestamp: usize,
}

/// Collect context for logging
///
/// TODO: get current cpu id and task id
pub(super) fn collect_context() -> LogContext {
    LogContext {
        cpu_id: 0,  // TODO: replace with current cpu id
        task_id: 0, // TODO: replace with current task id
        timestamp: timer::get_time(),
    }
}
