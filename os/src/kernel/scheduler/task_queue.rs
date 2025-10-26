use alloc::{sync::Arc, vec::Vec};

use crate::kernel::TaskStruct;

/// 任务队列
/// 用于存放任务
pub struct TaskQueue {
    queue: Vec<Arc<TaskStruct>>,
}

impl TaskQueue {
    /// 创建一个新的队列
    pub fn new() -> Self {
        TaskQueue { queue: Vec::new() }
    }

    /// 向运行队列添加任务
    pub fn add_task(&mut self, task: Arc<TaskStruct>) {
        self.queue.push(task);
    }

    /// 从运行队列中移除任务
    pub fn remove_task(&mut self, task: &TaskStruct) {
        self.queue.retain(|t| t.tid != task.tid);
    }

    /// 从运行队列中弹出一个任务
    pub fn pop_task(&mut self) -> Option<Arc<TaskStruct>> {
        if !self.queue.is_empty() {
            Some(self.queue.remove(0))
        } else {
            None
        }
    }
}
