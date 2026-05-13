//! 底层自旋锁
//!
//! 基于原子操作实现自旋锁机制，结合 `IntrGuard` 实现中断保护。
//! 不可重入 (即不能嵌套调用 RawSpinLock::lock())。
//!
//! # 泛型参数
//!
//! * `CPU` - 实现 `CpuOps` 的类型，默认使用 `ArchImpl`

use crate::arch::ArchImpl;
use crate::hal::CpuOps;
use crate::sync::intr_guard::IntrGuard;
use core::{
    hint,
    marker::PhantomData,
    sync::atomic::{AtomicBool, Ordering},
};

/// 自旋锁结构体，提供互斥访问临界区的能力。
pub struct RawSpinLock<CPU: CpuOps = ArchImpl> {
    lock: AtomicBool,
    _marker: PhantomData<CPU>,
}

impl<CPU: CpuOps> core::fmt::Debug for RawSpinLock<CPU> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RawSpinLock")
            .field("lock", &self.lock)
            .finish()
    }
}

impl<CPU: CpuOps> RawSpinLock<CPU> {
    pub const fn new() -> Self {
        RawSpinLock {
            lock: AtomicBool::new(false),
            _marker: PhantomData,
        }
    }

    /// 尝试获取自旋锁，并返回一个 RAII 保护器。
    ///
    /// 内部原子地获取锁，并在当前 CPU 禁用本地中断。
    pub fn lock(&self) -> RawSpinLockGuard<'_, CPU> {
        let guard = IntrGuard::<CPU>::new();

        while self
            .lock
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            hint::spin_loop();
        }

        RawSpinLockGuard {
            lock: self,
            intr_guard: guard,
        }
    }

    /// 尝试获取自旋锁，如果成功则返回 RAII 保护器，否则返回 None。
    ///
    /// 如果获取失败，会立即恢复中断状态（通过 Drop IntrGuard）。
    pub fn try_lock(&self) -> Option<RawSpinLockGuard<'_, CPU>> {
        let guard = IntrGuard::<CPU>::new();

        if self
            .lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(RawSpinLockGuard {
                lock: self,
                intr_guard: guard,
            })
        } else {
            None
        }
    }

    /// 仅释放锁标志。
    fn unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }

    /// 检查锁是否被占用 (仅用于调试/测试)
    #[cfg(test)]
    pub fn is_locked(&self) -> bool {
        self.lock.load(Ordering::Relaxed)
    }
}

/// 自动释放自旋锁和恢复中断状态的 RAII 结构体
pub struct RawSpinLockGuard<'a, CPU: CpuOps = ArchImpl> {
    lock: &'a RawSpinLock<CPU>,
    intr_guard: IntrGuard<CPU>,
}

use core::ops::Drop;

impl<CPU: CpuOps> Drop for RawSpinLockGuard<'_, CPU> {
    fn drop(&mut self) {
        self.lock.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        arch::intr::{are_interrupts_enabled, read_and_disable_interrupts, restore_interrupts},
        kassert, test_case,
    };

    static COUNTER: AtomicBool = AtomicBool::new(false);

    test_case!(test_raw_spin_lock_basic_lock_unlock, {
        let lock = RawSpinLock::<ArchImpl>::new();
        kassert!(!lock.is_locked());

        let guard = lock.lock();
        kassert!(lock.is_locked());

        drop(guard);
        kassert!(!lock.is_locked());
    });

    test_case!(test_raw_spin_lock_raii_release, {
        let lock = RawSpinLock::<ArchImpl>::new();

        {
            let _guard = lock.lock();
            kassert!(lock.is_locked());
        }

        kassert!(!lock.is_locked());
    });

    test_case!(test_raw_spin_lock_mutual_exclusion, {
        let lock = RawSpinLock::<ArchImpl>::new();

        let guard1 = lock.lock();
        kassert!(lock.is_locked());

        drop(guard1);

        let guard2 = lock.lock();
        let second_lock_failed = if lock.is_locked() { false } else { true };

        kassert!(!second_lock_failed);
        drop(guard2);
    });

    test_case!(test_interrupt_disable, {
        let initial_flags = unsafe { read_and_disable_interrupts() };
        unsafe { restore_interrupts(initial_flags | (1 << 1)) };
        kassert!(are_interrupts_enabled());

        let lock = RawSpinLock::<ArchImpl>::new();
        let guard = lock.lock();

        kassert!(!are_interrupts_enabled());
        kassert!(guard.intr_guard.was_enabled());

        drop(guard);
        kassert!(are_interrupts_enabled());

        unsafe { restore_interrupts(initial_flags) };
    });
}
