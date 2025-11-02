use crate::arch::timer;

pub struct LogContext { 
    pub cpu_id: usize,
    pub task_id: u32,
    pub timestamp: usize,
}

/// Collect context for logging
/// 
/// TODO: get current cpu id and task id
pub fn collect_context() -> LogContext {
    LogContext {
        cpu_id: 0,                          // TODO: replace with current cpu id
        task_id: 0,                         // TODO: replace with current task id
        timestamp: timer::get_time(),
    }
}