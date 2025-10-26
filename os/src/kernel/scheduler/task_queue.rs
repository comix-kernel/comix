use alloc::{sync::Arc, vec::Vec};

use crate::kernel::task::SharedTask;

/// 任务队列
/// 用于存放任务
pub struct TaskQueue {
    queue: Vec<SharedTask>,
}

impl TaskQueue {
    /// 创建一个新的队列
    pub fn new() -> Self {
        TaskQueue { queue: Vec::new() }
    }

    /// 向运行队列添加任务
    pub fn add_task(&mut self, task: SharedTask) {
        self.queue.push(task);
    }

    /// 从运行队列中移除任务
    pub fn remove_task(&mut self, task: &SharedTask) {
        self.queue.retain(|t| !Arc::ptr_eq(t, task));
    }

    /// 从运行队列中弹出一个任务
    pub fn pop_task(&mut self) -> Option<SharedTask> {
        if !self.queue.is_empty() {
            Some(self.queue.remove(0))
        } else {
            None
        }
    }

    /// 检查任务是否在队列中
    pub fn contains(&self, task: &SharedTask) -> bool {
        for t in &self.queue {
            if Arc::ptr_eq(t, task) {
                return true;
            }
        }
        false
    }

    /// 检查队列是否为空
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}
