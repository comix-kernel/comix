#![allow(dead_code)]
use crate::{
    kernel::{WaitQueue, current_cpu, yield_task},
    sync::raw_spin_lock::RawSpinLock,
};

/// 提供基本的互斥锁功能的结构体。
/// 使用睡眠等待的方式实现互斥锁，适用于可能长时间持有锁的场景。
/// 使用示例：
/// ```ignore
/// let mut mutex = Mutex::new();
/// mutex.lock(); // 获取锁
/// // 访问临界区
/// mutex.unlock(); // 释放锁
/// ```
pub struct Mutex {
    locked: bool,
    guard: RawSpinLock,
    queue: WaitQueue,
}

impl Mutex {
    /// 创建一个新的未锁定的 Mutex 实例。
    pub fn new() -> Self {
        Mutex {
            locked: false,
            guard: RawSpinLock::new(),
            queue: WaitQueue::new(),
        }
    }

    /// 获取锁。如果锁已被其他线程持有，则当前线程将进入睡眠状态，直到锁可用。
    pub fn lock(&mut self) {
        loop {
            let _g = self.guard.lock();
            if !self.locked {
                self.locked = true;
                return;
            }
            let current_task = current_cpu().lock().current_task.as_ref().unwrap().clone();
            // 在 enqueue 之前仍然持有 guard 是合理的，确保 wait queue 与 locked 字段的原子性
            self.queue.sleep(current_task);

            // 释放 guard，然后让出 CPU
            drop(_g);
            yield_task();
        }
    }

    /// 释放锁，并唤醒等待队列中的一个线程（如果有的话）。
    pub fn unlock(&mut self) {
        let _g = self.guard.lock();
        self.locked = false;
        self.queue.wake_up_one();
    }
}
