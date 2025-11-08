use crate::kernel::task::SharedTask;
use crate::kernel::{TaskQueue, sleep_task, wake_up};
use crate::sync::RawSpinLock;
use alloc::vec::Vec;

pub struct WaitQueue {
    tasks: TaskQueue,
    lock: RawSpinLock,
}

impl WaitQueue {
    pub fn new() -> Self {
        WaitQueue {
            tasks: TaskQueue::new(),
            lock: RawSpinLock::new(),
        }
    }

    /// 把任务加入等待队列，并在释放队列锁后调用 sleep_task（可能导致调度）
    pub fn sleep(&mut self, task: SharedTask) {
        {
            let _g = self.lock.lock();
            self.tasks.add_task(task.clone());
            // 在临界区内完成数据结构修改后立即释放 _g（leave scope）
        }
        // 在没有持有 wait-queue 锁的情况下调用调度相关操作
        sleep_task(task, true);
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
            wake_up(task.clone());
        }
    }

    /// 唤醒队首一个任务：在临界区内 pop，然后在临界区外唤醒
    pub fn wake_up_one(&mut self) {
        let maybe_task = {
            let _g = self.lock.lock();
            self.tasks.pop_task()
        };
        if let Some(t) = maybe_task {
            wake_up(t);
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
            wake_up(t);
        }
    }
}

unsafe impl Send for WaitQueue {}
unsafe impl Sync for WaitQueue {}
