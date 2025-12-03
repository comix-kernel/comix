//！ 调度器模块
//!
//！ 定义了调度器接口和相关功能
mod rr_scheduler;
mod task_queue;
mod wait_queue;

use lazy_static::lazy_static;

use crate::{
    arch::kernel::{context::Context, switch},
    kernel::{TaskStruct, current_task, scheduler::rr_scheduler::RRScheduler, task::SharedTask},
    pr_debug,
    sync::{SpinLock, SpinLockGuard},
};

pub use task_queue::TaskQueue;
pub use wait_queue::WaitQueue;

lazy_static! {
    pub static ref SCHEDULER: SpinLock<RRScheduler> = SpinLock::new(RRScheduler::new());
}

/// 上下文切换计划结构体
pub(crate) struct SwitchPlan {
    pub old: *mut Context,
    pub new: *const Context,
}

/// 调度器接口定义
/// 调度器负责决策和准备下一个任务的运行。
/// 具体来说，它负责以下几项工作：
/// 1. 决策： 根据优先级、时间片、调度策略等算法，从运行队列中选择下一个要执行的任务。
/// 2. 队列维护： 维护任务的运行队列（Run Queue）。
pub trait Scheduler {
    /// 构造函数
    fn new() -> Self;
    /// 添加任务到调度器
    /// 参数:
    /// * `task`: 需要添加的任务
    fn add_task(&mut self, task: SharedTask);
    /// 选择下一个要运行的任务
    /// # 返回值
    /// 如果要切换到下一个任务，返回切换计划 SwitchPlan；否则返回 None
    fn next_task(&mut self) -> Option<SwitchPlan>;
    /// 任务阻塞
    /// 修改任务状态并从运行队列中移除
    /// 参数:
    /// * `task`: 需要阻塞的任务
    /// * `receive_signal`: 是否可被信号中断
    /// 注意: 该函数仅设置状态，不负责切换任务
    fn sleep_task(&mut self, task: SharedTask, receive_signal: bool);
    /// 唤醒任务
    /// 修改任务状态并将其添加到运行队列
    /// 参数:
    /// * `task`: 需要唤醒的任务
    fn wake_up(&mut self, task: SharedTask);
    /// 任务终止
    /// 修改任务状态并从调度器中移除
    /// 参数:
    /// * `task`: 需要终止的任务
    fn exit_task(&mut self, task: SharedTask);
    /// 带保护地阻塞任务
    /// 修改任务状态并从运行队列中移除
    /// 参数:
    /// * `task`: 需要阻塞的任务（带锁保护）
    /// * `stask`: 需要阻塞的任务（共享指针）
    /// * `receive_signal`: 是否可被信号中断
    /// HACK: 这个函数被设计用来避免信号处理过程中丢失唤醒的问题。
    ///       尽量不要使用该函数，除非你非常清楚自己在做什么
    fn sleep_task_with_guard(
        &mut self,
        task: &mut SpinLockGuard<'_, TaskStruct>,
        stask: SharedTask,
        receive_signal: bool,
    );
}

/// 执行一次调度操作，切换到下一个任务
pub fn schedule() {
    let plan = {
        let mut sched = SCHEDULER.lock();
        // NOTE: next_task 内部会更新 current_task 与 current_memory_space 并切换页表
        sched.next_task()
    };

    if let Some(plan) = plan {
        pr_debug!("Switched to task {}", current_task().lock().tid);
        // SAFETY: prepare_switch 生成的切换计划中的指针均合法
        unsafe { switch(plan.old, plan.new) };
        // 通常不会立即返回；返回时再继续当前上下文后续逻辑
    }
}

/// 主动放弃 CPU
/// 切换到下一个任务
/// 如果调用该函数的任务仍可运行，将被放回运行队列末尾，等待下一次调度
pub fn yield_task() {
    schedule();
}

/// 任务阻塞
/// 修改任务状态并从运行队列中移除
/// 参数:
/// * `task`: 需要阻塞的任务
/// * `receive_signal`: 是否可被信号中断
/// 注意: 该函数仅设置状态，不负责切换任务
pub fn sleep_task_with_block(task: SharedTask, receive_signal: bool) {
    SCHEDULER.lock().sleep_task(task, receive_signal);
}

/// 唤醒任务
/// 修改任务状态并将其添加到运行队列
/// 参数:
/// * `task`: 需要唤醒的任务
pub fn wake_up_with_block(task: SharedTask) {
    SCHEDULER.lock().wake_up(task);
}

/// 任务终止
/// 修改任务状态并从调度器中移除
/// 参数:
/// * `task`: 需要终止的任务
pub fn exit_task_with_block(task: SharedTask) {
    SCHEDULER.lock().exit_task(task);
}

/// 带保护地阻塞任务
/// 修改任务状态并从运行队列中移除
/// 参数:
/// * `task`: 需要阻塞的任务（带锁保护）
/// * `stask`: 需要阻塞的任务（共享指针）
/// * `receive_signal`: 是否可被信号中断
/// HACK: 这个函数被设计用来避免信号处理过程中丢失唤醒的问题。
///       尽量不要使用该函数，除非你非常清楚自己在做什么
pub fn sleep_task_with_guard_and_block(
    task: &mut SpinLockGuard<'_, TaskStruct>,
    stask: SharedTask,
    receive_signal: bool,
) {
    SCHEDULER
        .lock()
        .sleep_task_with_guard(task, stask, receive_signal);
}
