//! 任务状态定义

/// 任务状态
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    /// 可执行。正在执行或可被调度执行
    Running,
    /// 停止。没有也不能执行
    Stopped,
    /// 等待可中断的事件。可以被信号等中断唤醒
    Interruptible,
    /// 等待不可中断的事件。不能被信号等中断唤醒
    Uninterruptible,
}
