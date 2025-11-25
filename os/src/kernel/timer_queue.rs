//! 定时器队列模块
//!
//! 该模块实现了一个简单的定时器队列，用于管理和调度定时任务。

use alloc::collections::btree_map::BTreeMap;

use crate::{kernel::SharedTask, sync::SpinLock};

lazy_static::lazy_static! {
    /// 全局定时器队列实例
    /// 使用硬件时钟周期数作为时间单位
    pub static ref TIMER_QUEUE: SpinLock<TimerQueue> = SpinLock::new(TimerQueue::new());
}

/// 定时器队列，用于管理定时任务
pub struct TimerQueue {
    /// 以触发时间为键，任务为值的有序映射
    queue: BTreeMap<usize, SharedTask>,
}

impl TimerQueue {
    /// 创建一个新的定时器队列
    pub fn new() -> Self {
        Self {
            queue: BTreeMap::new(),
        }
    }

    /// 向队列中添加一个定时任务
    /// # 参数:
    /// - `trigger_time`: 任务触发的时间点
    /// - `task`: 需要执行的任务
    pub fn push(&mut self, trigger_time: usize, task: SharedTask) {
        self.queue.insert(trigger_time, task);
    }

    /// 弹出已到期的任务
    /// # 参数:
    /// - `current_time`: 当前时间点
    /// # 返回值:
    /// - 已到期的任务（如果有）
    pub fn pop_due_task(&mut self, current_time: usize) -> Option<SharedTask> {
        if let Some((&trigger_time, _)) = self.queue.iter().next() {
            if trigger_time <= current_time {
                return self.queue.remove(&trigger_time);
            }
        }
        None
    }
}
