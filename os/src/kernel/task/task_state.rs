//! 任务状态定义

/// 任务状态
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    /// 可执行。正在执行或可被调度执行
    Running,
    /// 被信号暂停。任务暂停执行，直到收到继续信号
    Stopped,
    /// 等待可中断的事件。可以被信号等中断唤醒
    Interruptible,
    /// 等待不可中断的事件。不能被信号等中断唤醒
    Uninterruptible,
    /// 僵尸状态。任务已终止，但其父进程尚未回收其资源
    Zombie,
}
