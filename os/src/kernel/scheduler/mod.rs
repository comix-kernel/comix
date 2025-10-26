#![allow(dead_code)]
mod rr_scheduler;
mod task_queue;

use lazy_static::lazy_static;

use crate::{
    kernel::{scheduler::rr_scheduler::RRScheduler, task::SharedTask},
    sync::spin_lock::SpinLock,
};

lazy_static! {
    pub static ref SCHEDULER: SpinLock<RRScheduler> = SpinLock::new(RRScheduler::new());
}

/// 调度器接口定义
pub trait Scheduler {
    /// 构造函数
    fn new() -> Self;
    /// 核心调度循环
    fn schedule(&mut self);
    /// 添加任务到调度器
    fn add_task(&mut self, task: SharedTask);
    /// 选择下一个要运行的任务
    fn next_task(&mut self) -> SharedTask;
    /// 主动放弃 CPU
    fn yield_task(&mut self);
    /// 任务阻塞（由调用者指定任务）
    fn sleep_task(&mut self, task: SharedTask);
    /// 唤醒任务（由调用者指定任务）
    fn wake_up(&mut self, task: SharedTask);
    /// 任务终止（由调用者指定任务）
    fn exit_task(&mut self, task: SharedTask, code: i32);
}
