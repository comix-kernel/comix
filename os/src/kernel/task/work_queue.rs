//! 任务工作队列模块
//!
//! 任务工作队列用于延迟执行某些任务相关的操作，以避免在关键路径上执行耗时操作。
//! 例如，当任务终止时，我们不希望立即清理任务资源，而是将清理操作放入工作队列中，
//! 由专门的工作线程在合适的时机执行。
#![allow(dead_code)]

use alloc::{collections::vec_deque::VecDeque, vec::Vec};

use crate::{
    kernel::{
        SharedTask, TaskState, current_task, sleep_task_with_block, wake_up_with_block, yield_task,
    },
    sync::SpinLock,
};

lazy_static::lazy_static! {
    pub static ref GLOBAL_WORK_QUEUE: SpinLock<WorkQueue> = SpinLock::new(WorkQueue::new());
}

/// 工作项结构体
pub struct WorkItem {
    /// 任务函数
    pub task: fn(),
}

impl WorkItem {
    /// 创建一个新的工作项
    pub fn new(task: fn()) -> Self {
        WorkItem { task }
    }
}

/// 工作队列结构体
pub struct WorkQueue {
    /// 当前处于休眠状态的工作线程数量
    sleeping: usize,
    /// 工作线程列表
    worker: Vec<SharedTask>,
    /// 待处理的工作项队列
    work_queue: VecDeque<WorkItem>,
}

impl WorkQueue {
    /// 创建一个新的工作队列实例
    pub fn new() -> Self {
        WorkQueue {
            worker: Vec::new(),
            work_queue: VecDeque::new(),
            sleeping: 0,
        }
    }

    /// 将工作项加入工作队列，并唤醒工作线程
    pub fn schedule_work(&mut self, work: WorkItem) {
        self.work_queue.push_back(work);
        if self.sleeping > 0 {
            for task in &self.worker {
                if task.lock().state == TaskState::Interruptible {
                    wake_up_with_block(task.clone());
                    break;
                }
            }
        }
    }

    /// 添加工作线程到工作队列
    pub fn add_worker(&mut self, task: SharedTask) {
        self.worker.push(task);
    }
}

/// 工作线程主函数
pub fn kworker() {
    GLOBAL_WORK_QUEUE.lock().add_worker(current_task());
    loop {
        let mut queue = GLOBAL_WORK_QUEUE.lock();

        if let Some(work) = queue.work_queue.pop_front() {
            (work.task)();
        } else {
            queue.sleeping += 1;
            sleep_task_with_block(current_task(), true);
            drop(queue);
            yield_task();
            GLOBAL_WORK_QUEUE.lock().sleeping -= 1;
        }
    }
}
