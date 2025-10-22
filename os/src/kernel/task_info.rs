#![allow(dead_code)]

use crate::arch::kernel::context::Context;
/// 关于任务的管理信息
/// 存放与调度器、任务状态、队列相关的、需要高频访问和修改的数据。
/// 主要由调度器子系统使用。
pub struct TaskInfo {
    /// 任务的上下文信息，用于任务切换
    pub context: Context,
    /// 任务的抢占计数器，表示当前任务被禁止抢占的次数
    /// 当该值大于0时，表示任务处于不可抢占状态
    pub preempt_count: usize,
    /// 任务的优先级，数值越小优先级越高
    pub priority: u8,
    /// 任务所在的处理器id
    pub processor_id: usize,
    /// 任务当前的状态
    pub state: TaskState,
    /// 任务的id
    pub tid: usize,
    /// 任务的所属进程id
    /// NOTE: 由于采用了统一的任务模型，一个任务组内任务的 pid 是相同的，等于父任务的 pid 而父任务的 pid 等于自己的 tid
    pub pid: usize,
}

/// 任务状态
pub enum TaskState {
    /// 可执行。正在执行或可被调度执行
    Running,
    /// 停止。没有也不能执行
    Stopped,
    /// 等待可中断的事件。可以被信号等中断唤醒
    Interruptable,
    /// 等待不可中断的事件。不能被信号等中断唤醒
    Uninterruptable,
}
