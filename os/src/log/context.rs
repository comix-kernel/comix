//! 日志上下文收集
//!
//! 该模块为每个日志条目收集**上下文信息**（CPU ID、任务 ID、时间戳）。

use crate::arch::{kernel::cpu::cpu_id, timer};

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
/// - **CPU ID**: 通过 `arch::kernel::cpu::cpu_id()` 实现
/// - **任务 ID**: 通过当前任务的 tid 字段获取（如果存在任务）
pub(super) fn collect_context() -> LogContext {
    // 获取 CPU ID
    let cpu_id = cpu_id();

    // 尝试获取当前任务的 tid
    // 注意：在早期启动或中断上下文中可能没有当前任务
    // 更重要的是：如果当前已经在持有 task lock 的上下文中（例如 wait4），
    // 再次尝试获取锁会导致死锁。因此这里必须使用 try_lock。
    let task_id = crate::kernel::current_cpu()
        .try_lock()
        .and_then(|cpu| cpu.current_task.as_ref().cloned())
        .and_then(|task| task.try_lock().map(|t| t.tid))
        .unwrap_or(0);

    LogContext {
        cpu_id,
        task_id,
        timestamp: timer::get_time(),
    }
}
