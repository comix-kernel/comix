#![allow(dead_code)]
use crate::{
    kernel::{WaitQueue, current_cpu, yield_task},
    sync::raw_spin_lock::RawSpinLock,
};

pub struct SleepLock {
    locked: bool,
    guard: RawSpinLock,
    queue: WaitQueue,
}

impl SleepLock {
    pub fn new() -> Self {
        SleepLock {
            locked: false,
            guard: RawSpinLock::new(),
            queue: WaitQueue::new(),
        }
    }

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
            // 释放 guard，然后睡眠；被唤醒后再次循环，重新获取 guard
            drop(_g);
            yield_task();
        }
    }

    pub fn unlock(&mut self) {
        let _g = self.guard.lock();
        self.locked = false;
        self.queue.wake_up_one();
    }
}
