//! 互斥锁
//!
//! 睡眠互斥锁，当锁被占用时会让出 CPU 而不是自旋等待。
//! 适用于临界区较长的场景。
//!
//! # 泛型参数
//!
//! * `T` - 被保护的数据类型
//! * `CPU` - 实现 `CpuOps` 的类型，默认使用 `ArchImpl`

#![allow(dead_code)]
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::arch::ArchImpl;
use crate::arch::CpuOps;
use crate::kernel::{WaitQueue, current_task, yield_task};
use crate::sync::SpinLock;
use crate::sync::{raw_spin_lock::RawSpinLock, raw_spin_lock::RawSpinLockGuard};

/// 互斥锁
pub struct Mutex<T, CPU: CpuOps = ArchImpl> {
    locked: AtomicBool,
    guard: RawSpinLock<CPU>,
    queue: SpinLock<WaitQueue, CPU>,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send, CPU: CpuOps> Send for Mutex<T, CPU> {}
unsafe impl<T: Send, CPU: CpuOps> Sync for Mutex<T, CPU> {}

impl<T, CPU: CpuOps> Mutex<T, CPU> {
    pub fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            guard: RawSpinLock::new(),
            queue: SpinLock::new(WaitQueue::new()),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, T, CPU> {
        loop {
            let spin = self.guard.lock();
            if !self.locked.swap(true, Ordering::Acquire) {
                let data_ref = unsafe { &mut *self.data.get() };
                return MutexGuard {
                    mutex: self as *const _,
                    _spin: spin,
                    data: data_ref,
                };
            }
            let current = current_task();
            self.queue.lock().sleep(current);
            drop(spin);
            yield_task();
        }
    }
}

pub struct MutexGuard<'a, T, CPU: CpuOps = ArchImpl> {
    mutex: *const Mutex<T, CPU>,
    _spin: RawSpinLockGuard<'a, CPU>,
    data: &'a mut T,
}

impl<T, CPU: CpuOps> core::ops::Deref for MutexGuard<'_, T, CPU> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T, CPU: CpuOps> core::ops::DerefMut for MutexGuard<'_, T, CPU> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<T, CPU: CpuOps> Drop for MutexGuard<'_, T, CPU> {
    fn drop(&mut self) {
        let m = unsafe { &*self.mutex };
        m.locked.store(false, Ordering::Release);
        m.queue.lock().wake_up_all();
    }
}
