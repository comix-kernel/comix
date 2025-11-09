//! 日志上下文收集
//!
//! 该模块为每个日志条目收集**上下文信息**（CPU ID、任务 ID、时间戳）。

use crate::arch::timer;

/// 日志条目的上下文信息
pub(super) struct LogContext {
    /// 生成此日志的 CPU ID
    pub(super) cpu_id: usize,
    /// 生成此日志的任务/进程 ID
    pub(super) task_id: u32,
    /// 创建日志时的时间戳
    pub(super) timestamp: usize,
}

/// 为新的日志条目收集上下文信息
///
/// # 实现状态
///
/// - **时间戳**: 已通过 `arch::timer::get_time()` 实现
/// - **CPU ID**: 待办事项 (TODO) - 需要多核支持实现
/// - **任务 ID**: 待办事项 (TODO) - 需要任务管理集成
pub(super) fn collect_context() -> LogContext {
    LogContext {
        cpu_id: 0,  // TODO: 替换为当前 CPU ID
        task_id: 0, // TODO: 替换为当前任务 ID
        timestamp: timer::get_time(),
    }
}
