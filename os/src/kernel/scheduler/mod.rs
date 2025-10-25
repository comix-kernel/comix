#![allow(dead_code)]
mod rr_scheduler;
use alloc::vec::Vec;

use crate::kernel::task::Task;

/// 调度器接口定义
pub trait Scheduler {
    // 构造函数
    fn new() -> Self;

    // 核心调度循环
    fn schedule(&mut self); // 核心调度逻辑 (调用 switch_to)

    // 任务管理
    fn add_task(&mut self, task: Task); // 任务首次创建或从阻塞队列返回
    fn next_task(&mut self) -> Task; // 选择下一个要运行的任务

    // 状态转换 (由任务自身或中断调用)
    fn yield_task(&mut self); // 主动放弃 CPU
    fn sleep_task(&mut self, wq: &WaitQueue); // 任务阻塞
    fn wake_up(&mut self, wq: &WaitQueue); // 唤醒任务
    fn exit_task(&mut self, code: i32); // 任务终止
}

/// 简单的等待队列结构体
/// 用于任务阻塞和唤醒
pub struct WaitQueue {
    queue: Vec<Task>,
}

impl WaitQueue {
    /// 创建一个新的等待队列
    pub fn new() -> Self {
        WaitQueue { queue: Vec::new() }
    }

    /// 向等待队列添加任务
    pub fn add_task(&mut self, task: Task) {
        self.queue.push(task);
    }

    /// 从等待队列中移除任务
    pub fn remove_task(&mut self, task: &Task) {
        self.queue.retain(|t| t.tid != task.tid);
    }

    /// 从等待队列中弹出一个任务
    pub fn pop_task(&mut self) -> Option<Task> {
        if !self.queue.is_empty() {
            Some(self.queue.remove(0))
        } else {
            None
        }
    }
}

/// 运行队列
/// 用于存放可运行的任务
pub struct RunQueue {
    queue: Vec<Task>,
}

impl RunQueue {
    /// 创建一个新的运行队列
    pub fn new() -> Self {
        RunQueue { queue: Vec::new() }
    }

    /// 向运行队列添加任务
    pub fn add_task(&mut self, task: Task) {
        self.queue.push(task);
    }

    /// 从运行队列中移除任务
    pub fn remove_task(&mut self, task: &Task) {
        self.queue.retain(|t| t.tid != task.tid);
    }

    /// 从运行队列中弹出一个任务
    pub fn pop_task(&mut self) -> Option<Task> {
        if !self.queue.is_empty() {
            Some(self.queue.remove(0))
        } else {
            None
        }
    }
}
