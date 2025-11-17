#![allow(dead_code)]
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::kernel::{WaitQueue, current_cpu, yield_task};
use crate::sync::SpinLock;
use crate::sync::{raw_spin_lock::RawSpinLock, raw_spin_lock::RawSpinLockGuard};

/// 互斥锁
pub struct Mutex<T> {
    locked: AtomicBool,
    guard: RawSpinLock,
    queue: SpinLock<WaitQueue>,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for Mutex<T> {}
unsafe impl<T: Send> Sync for Mutex<T> {}

impl<T> Mutex<T> {
    pub fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            guard: RawSpinLock::new(),
            queue: SpinLock::new(WaitQueue::new()),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, T> {
        loop {
            // 先自旋获取内部短锁
            let spin = self.guard.lock();
            // 尝试占用
            if !self.locked.swap(true, Ordering::Acquire) {
                // 成功，占用了
                let data_ref = unsafe { &mut *self.data.get() };
                return MutexGuard {
                    mutex: self as *const _,
                    _spin: spin,
                    data: data_ref,
                };
            }
            // 已被占用：把当前任务挂到等待队列
            let current = current_cpu()
                .lock()
                .current_task
                .as_ref()
                .expect("mutex::lock: no current task")
                .clone();
            self.queue.lock().sleep(current);
            // 释放短锁后让调度器切走
            drop(spin);
            yield_task();
            // 醒来后重试
        }
    }
}

pub struct MutexGuard<'a, T> {
    mutex: *const Mutex<T>,
    _spin: RawSpinLockGuard<'a>, // 保持内部互斥到 drop
    data: &'a mut T,
}

impl<T> core::ops::Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T> core::ops::DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        // 仍持有 _spin，自然是互斥的
        let m = unsafe { &*self.mutex };
        m.locked.store(false, Ordering::Release);
        m.queue.lock().wake_up_all();
    }
}
