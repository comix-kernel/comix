//! Raw spin lock without guard, specifically designed for Global Allocator integration
//!
//! This module provides a spin lock implementation that integrates with `lock_api::RawMutex`
//! for use with the `talc` allocator's `Talck` type.
//!
//! # Key Differences from `RawSpinLock`
//!
//! - Implements `lock_api::RawMutex` trait
//! - Does not return a Guard from `lock()` method
//! - Stores interrupt state internally using `AtomicUsize`
//! - Unlock operation restores the interrupt state
//!
//! # Interrupt Safety
//!
//! This lock provides interrupt protection to prevent deadlocks when:
//! - A thread holds the allocator lock
//! - An interrupt occurs on the same CPU
//! - The interrupt handler tries to allocate memory
//!
//! Without interrupt protection, this would cause a deadlock.

use crate::arch::intr::{read_and_disable_interrupts, restore_interrupts};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// 自旋锁结构体，不返回 Guard，集成了中断状态保存与恢复功能。
pub struct RawSpinLockWithoutGuard {
    locked: AtomicBool,
    saved_intr_flags: AtomicUsize,
}

impl RawSpinLockWithoutGuard {
    /// 创建一个新的 RawSpinLockWithoutGuard 实例。
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
            saved_intr_flags: AtomicUsize::new(0),
        }
    }
}

unsafe impl lock_api::RawMutex for RawSpinLockWithoutGuard {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self::new();

    type GuardMarker = lock_api::GuardNoSend;

    /// 获取锁，禁用中断并保存状态。
    fn lock(&self) {
        // 1. Disabldde interrupts and save the flags
        let flags = unsafe { read_and_disable_interrupts() };

        // 2. Spin until we acquire the lock
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }

        // 3. Store the interrupt flags for later restoration
        self.saved_intr_flags.store(flags, Ordering::Release);
    }

    /// 尝试获取锁，成功则禁用中断并保存状态。
    fn try_lock(&self) -> bool {
        // 1. Disable interrupts and save the flags
        let flags = unsafe { read_and_disable_interrupts() };

        // 2. Try to acquire the lock
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            // 3. Success: store the interrupt flags
            self.saved_intr_flags.store(flags, Ordering::Release);
            true
        } else {
            // 4. Failed: restore interrupts immediately
            unsafe { restore_interrupts(flags) };
            false
        }
    }

    /// 释放锁，恢复之前的中断状态。
    unsafe fn unlock(&self) {
        // 1. Load the saved interrupt flags
        let flags = self.saved_intr_flags.load(Ordering::Acquire);

        // 2. Release the lock
        self.locked.store(false, Ordering::Release);

        // 3. Restore the interrupt state
        unsafe { restore_interrupts(flags) };
    }
}

// Safety: 锁的使用保证了多线程环境下的正确性
unsafe impl Send for RawSpinLockWithoutGuard {}
unsafe impl Sync for RawSpinLockWithoutGuard {}

#[cfg(test)]
mod tests {
    use lock_api::RawMutex;

    use super::*;
    use crate::{kassert, test_case};

    // 使用 lock_api::Mutex 包装进行基本功能测试
    test_case!(test_mutex_wrapper_guard_basic, {
        let m = lock_api::Mutex::<RawSpinLockWithoutGuard, usize>::new(0);

        {
            let mut g = m.lock();
            *g = 42;
        } // guard drop -> 解锁

        {
            let g = m.lock();
            kassert!(*g == 42);
        }
    });

    // try_lock 成功/失败与解锁后的可重入获取
    test_case!(test_try_lock_and_unlock_roundtrip, {
        let raw = RawSpinLockWithoutGuard::new();

        // 第一次尝试应成功
        let ok = raw.try_lock();
        kassert!(ok);

        // 持有锁时第二次尝试应失败
        let fail = raw.try_lock();
        kassert!(!fail);

        // 解锁
        unsafe {
            raw.unlock();
        }

        // 解锁后应可再次获取
        let ok2 = raw.try_lock();
        kassert!(ok2);

        // 再次解锁，避免影响其他用例
        unsafe {
            raw.unlock();
        }
    });

    // lock() 获取（在未持锁情况下不应阻塞），随后解锁
    test_case!(test_lock_then_unlock, {
        let raw = RawSpinLockWithoutGuard::new();

        // 未持锁情况下 lock() 应立即返回并持有锁
        raw.lock();

        // 此时 try_lock 应失败
        let fail = raw.try_lock();
        kassert!(!fail);

        // 解锁后应可再次获取
        unsafe {
            raw.unlock();
        }
        let ok = raw.try_lock();
        kassert!(ok);
        unsafe {
            raw.unlock();
        }
    });
}
