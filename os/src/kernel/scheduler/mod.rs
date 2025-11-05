#![allow(dead_code)]
mod rr_scheduler;
mod task_queue;
mod wait_queue;

use lazy_static::lazy_static;

use crate::{
    arch::kernel::{context::Context, switch},
    kernel::{scheduler::rr_scheduler::RRScheduler, task::SharedTask},
    sync::spin_lock::SpinLock,
};

pub use task_queue::TaskQueue;
pub use wait_queue::WaitQueue;

lazy_static! {
    pub static ref SCHEDULER: SpinLock<RRScheduler> = SpinLock::new(RRScheduler::new());
}

/// 上下文切换计划结构体
pub struct SwitchPlan {
    pub old: *mut Context,
    pub new: *const Context,
}

/// 调度器接口定义
pub trait Scheduler {
    /// 构造函数
    fn new() -> Self;
    /// 准备一次上下文切换，返回切换计划
    /// HACK: 因为现在如果在 Scheduler 中直接 switch 会导致 SCHEDULER 的锁不能释放
    ///       有没有更好的方法？
    fn prepare_switch(&mut self) -> Option<SwitchPlan>;
    /// 添加任务到调度器
    fn add_task(&mut self, task: SharedTask);
    /// 选择下一个要运行的任务
    fn next_task(&mut self) -> Option<SharedTask>;
    /// 任务阻塞
    /// 修改任务状态并从运行队列中移除
    fn sleep_task(&mut self, task: SharedTask, receive_signal: bool);
    /// 唤醒任务
    /// 修改任务状态并将其添加到运行队列
    fn wake_up(&mut self, task: SharedTask);
    /// 任务终止
    /// 修改任务状态并从调度器中移除
    fn exit_task(&mut self, task: SharedTask, code: i32);
}

/// 执行一次调度操作，切换到下一个任务
pub fn schedule() {
    let plan = {
        let mut sched = SCHEDULER.lock();
        sched.prepare_switch()
    };

    if let Some(plan) = plan {
        unsafe { switch(plan.old, plan.new) };
        // 通常不会立即返回；返回时再继续当前上下文后续逻辑
    }
}

/// 主动放弃 CPU
pub fn yield_task() {
    schedule();
}

/// 任务阻塞
/// 修改任务状态并从运行队列中移除
pub fn sleep_task(task: SharedTask, receive_signal: bool) {
    SCHEDULER.lock().sleep_task(task, receive_signal);
}

/// 唤醒任务
/// 修改任务状态并将其添加到运行队列
pub fn wake_up(task: SharedTask) {
    SCHEDULER.lock().wake_up(task);
}

/// 任务终止
/// 修改任务状态并从调度器中移除
pub fn exit_task(task: SharedTask, code: i32) {
    SCHEDULER.lock().exit_task(task, code);
}
