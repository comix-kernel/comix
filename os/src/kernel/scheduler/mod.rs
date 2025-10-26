#![allow(dead_code)]
mod rr_scheduler;
mod task_queue;

use alloc::sync::Arc;
use lazy_static::lazy_static;

use crate::{
    kernel::{TaskStruct, scheduler::rr_scheduler::RRScheduler},
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
    fn add_task(&mut self, task: Arc<TaskStruct>);
    /// 选择下一个要运行的任务
    fn next_task(&mut self) -> Arc<TaskStruct>;
    /// 主动放弃 CPU
    fn yield_task(&mut self);
    /// 任务阻塞
    fn sleep_task(&mut self);
    /// 唤醒任务
    fn wake_up(&mut self);
    /// 任务终止
    fn exit_task(&mut self, code: i32);
}
