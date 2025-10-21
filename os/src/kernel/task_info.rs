/// 关于任务的管理信息
/// 存放与调度器、任务状态、队列相关的、需要高频访问和修改的数据。
/// 主要由调度器子系统使用。
pub struct TaskInfo {
    /// 任务的抢占计数器，表示当前任务被禁止抢占的次数
    /// 当该值大于0时，表示任务处于不可抢占状态
    preempt_count: usize,
    /// 任务的优先级，数值越小优先级越高
    priority: u8,
    /// 任务所在的处理器id
    processor_id: usize,
    /// 任务当前的状态
    state: TaskState,
    /// 任务的id
    tid: usize,
}

/// 任务状态
pub enum TaskState {
    /// 可执行。正在执行或可被调度执行
    RUNNING,
    /// 停止。没有也不能执行
    STOPPED,
    /// 等待可中断的事件。可以被信号等中断唤醒
    INTERRUPTABLE,
    /// 等待不可中断的事件。不能被信号等中断唤醒
    UNINTERRUPTABLE,
}