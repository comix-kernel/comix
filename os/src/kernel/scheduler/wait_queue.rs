//! 等待队列模块
//!
//! 定义了等待队列结构体及其相关操作
use crate::kernel::task::SharedTask;
use crate::kernel::{TaskQueue, sleep_task_with_block, wake_up_with_block, yield_task};
use crate::sync::RawSpinLock;
use alloc::vec::Vec;

/// 等待队列结构体
/// 用于管理等待某些事件的任务列表
/// 提供将任务加入等待队列、从队列中唤醒任务等功能
/// 内部使用任务队列和自旋锁来保证线程安全
/// 使用示例：
/// ```ignore
/// let mut wait_queue = WaitQueue::new();
/// wait_queue.sleep(task); // 将任务加入等待队列并阻塞
/// wait_queue.wake_up(&task); // 唤醒指定任务
/// wait_queue.wake_up_one(); // 唤醒队首任务
/// wait_queue.wake_up_all(); // 唤醒所有任务
/// ```
#[derive(Debug)]
pub struct WaitQueue {
    tasks: TaskQueue,
    lock: RawSpinLock,
}

impl WaitQueue {
    /// 创建一个新的等待队列实例
    pub fn new() -> Self {
        WaitQueue {
            tasks: TaskQueue::new(),
            lock: RawSpinLock::new(),
        }
    }

    /// 把任务加入等待队列，并调用 sleep_task（不会导致调度）
    pub fn sleep(&mut self, task: SharedTask) {
        let _g = self.lock.lock();
        self.tasks.add_task(task.clone());
        sleep_task_with_block(task, true);
    }

    /// 从等待队列中移除指定任务并在锁释放后唤醒
    pub fn wake_up(&mut self, task: &SharedTask) {
        let should_wake = {
            let _g = self.lock.lock();
            if self.tasks.contains(task) {
                self.tasks.remove_task(task);
                true
            } else {
                false
            }
        };
        if should_wake {
            wake_up_with_block(task.clone());
        }
    }

    /// 唤醒队首一个任务：在临界区内 pop，然后在临界区外唤醒
    pub fn wake_up_one(&mut self) {
        let maybe_task = {
            let _g = self.lock.lock();
            self.tasks.pop_task()
        };
        if let Some(t) = maybe_task {
            wake_up_with_block(t);
        }
    }

    /// 唤醒队列中所有任务：一次性把要唤醒的任务收集出来，释放锁后逐个唤醒
    pub fn wake_up_all(&mut self) {
        let mut to_wake: Vec<SharedTask> = Vec::new();
        {
            let _g = self.lock.lock();
            while let Some(t) = self.tasks.pop_task() {
                to_wake.push(t);
            }
        }
        for t in to_wake {
            wake_up_with_block(t);
        }
    }

    /// 将任务加入等待队列（不阻塞）
    pub fn add_task(&mut self, task: SharedTask) {
        let _g = self.lock.lock();
        self.tasks.add_task(task);
    }

    /// 从等待队列中移除指定任务（不唤醒）
    pub fn remove_task(&mut self, task: &SharedTask) {
        let _g = self.lock.lock();
        self.tasks.remove_task(task);
    }

    /// 检查任务是否在队列中
    pub fn contains(&self, task: &SharedTask) -> bool {
        let _g = self.lock.lock();
        self.tasks.contains(task)
    }

    /// 检查等待队列是否为空
    pub fn is_empty(&self) -> bool {
        let _g = self.lock.lock();
        self.tasks.is_empty()
    }

    /// 原子地检查条件并睡眠（用于防止 lost wakeup）
    /// check_fn 在持有锁时被调用，如果返回 true 则不睡眠
    pub fn sleep_if<F>(&mut self, task: SharedTask, check_fn: F) -> bool
    where
        F: FnOnce() -> bool,
    {
        let _g = self.lock.lock();
        if check_fn() {
            return false; // 条件满足，不睡眠
        }
        self.tasks.add_task(task.clone());
        sleep_task_with_block(task, true);
        true // 已睡眠
    }
}

// SAFETY:
// WaitQueue 内部使用 RawSpinLock 来保护任务队列的并发访问
// 因此 WaitQueue 本身是线程安全的，可以在多线程环境中共享
unsafe impl Send for WaitQueue {}
unsafe impl Sync for WaitQueue {}
