//! 定时器队列模块
//!
//! 该模块实现了一个简单的定时器队列，用于管理和调度定时任务。

use alloc::{collections::btree_map::BTreeMap, sync::Arc};

use crate::{kernel::SharedTask, sync::SpinLock, vfs::TimeSpec};

lazy_static::lazy_static! {
    /// 全局等待队列实例
    /// 使用硬件时钟周期数作为时间单位
    /// 在定时器触发时唤醒任务
    pub static ref TIMER_QUEUE: SpinLock<TimerQueue> = SpinLock::new(TimerQueue::new());
    /// 定时器队列
    /// 使用硬件时钟周期数作为时间单位
    /// 在定时器触发时向任务发送对应信号
    pub static ref TIMER: SpinLock<TimerEntries> = SpinLock::new(TimerEntries::new());
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
    pub fn push(&mut self, mut trigger_time: usize, task: SharedTask) {
        while self.queue.contains_key(&trigger_time) {
            trigger_time += 1;
        }
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

    /// 移除指定任务
    /// # 参数:
    /// - `task`: 需要移除的任务
    /// # 返回值:
    /// - 被移除的任务（如果存在）
    pub fn remove_task(&mut self, task: &SharedTask) -> Option<SharedTask> {
        let key = self.queue.iter().find_map(|(time, t)| {
            if Arc::ptr_eq(task, t) {
                Some(*time)
            } else {
                None
            }
        })?;
        self.queue.remove(&key)
    }
}

/// 定时器条目
pub struct TimerEntry {
    /// 信号编号
    pub sig: usize,
    /// 关联的任务
    pub task: SharedTask,
    /// 定时器周期
    pub it_interval: TimeSpec,
}

impl TimerEntry {
    /// 创建一个新的定时器条目
    pub fn new(sig: usize, task: SharedTask, it_interval: TimeSpec) -> Self {
        Self {
            sig,
            task,
            it_interval,
        }
    }
}

/// 定时器条目集合
pub struct TimerEntries {
    pub entries: BTreeMap<usize, TimerEntry>,
}

impl TimerEntries {
    /// 创建一个新的定时器条目集合
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// 向集合中添加一个定时器条目
    /// # 参数:
    /// - `trigger_time`: 触发时间点
    /// - `entry`: 定时器条目
    pub fn push(&mut self, mut trigger_time: usize, entry: TimerEntry) {
        while self.entries.contains_key(&trigger_time) {
            trigger_time += 1;
        }
        self.entries.insert(trigger_time, entry);
    }

    /// 弹出已到期的定时器条目
    /// # 参数:
    /// - `current_time`: 当前时间点
    /// # 返回值:
    /// - 已到期的定时器条目（如果有）
    pub fn pop_due_entry(&mut self, current_time: usize) -> Option<TimerEntry> {
        if let Some((&trigger_time, _)) = self.entries.iter().next() {
            if trigger_time <= current_time {
                return self.entries.remove(&trigger_time);
            }
        }
        None
    }

    /// 查找与指定任务关联的定时器条目
    /// # 参数:
    /// - `task`: 目标任务
    /// # 返回值:
    /// - 关联的定时器条目（如果存在）
    pub fn find_entry(&self, task: &SharedTask, sig: usize) -> Option<(&usize, &TimerEntry)> {
        for (time, entry) in self.entries.iter() {
            if Arc::ptr_eq(task, &entry.task) && entry.sig == sig {
                return Some((time, entry));
            }
        }
        None
    }

    /// 移除与指定任务关联的定时器条目
    /// # 参数:
    /// - `task`: 目标任务
    /// # 返回值:
    /// - 被移除的定时器条目（如果存在）
    pub fn remove_entry(&mut self, task: &SharedTask, sig: usize) -> Option<TimerEntry> {
        let key = self.find_entry(task, sig).map(|(time, _)| *time)?;
        self.entries.remove(&key)
    }
}
