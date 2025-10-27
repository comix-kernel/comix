use core::cell::UnsafeCell;

use crate::sync::raw_spin_lock::{RawSpinLock, RawSpinLockGuard};

/// 提供对数据的互斥访问的自旋锁结构体。
/// 内部包含一个 RawSpinLock 和一个 UnsafeCell 用于存储数据。
/// 使用示例：
/// ```ignore
/// let lock = SpinLock::new(0);
/// {
///     let mut guard = lock.lock(); // 获取锁
///     *guard += 1; // 访问和修改数据
/// } // 离开作用域，自动释放锁
/// ```
/// 注意：SpinLock 不是可重入的。
/// 当持有锁时，尝试再次获取锁将导致死锁。
/// 确保在同一线程中不会嵌套调用 SpinLock::lock()。
/// 此外，SpinLock 通过禁用中断来保护临界区，因此在持有锁时应避免长时间运行的操作，以防止影响系统响应性。
pub struct SpinLock<T> {
    raw_lock: RawSpinLock,
    data: UnsafeCell<T>,
}

impl<T> SpinLock<T> {
    pub const fn new(data: T) -> Self {
        SpinLock {
            raw_lock: RawSpinLock::new(),
            data: UnsafeCell::new(data),
        }
    }

    pub unsafe fn lock(&self) -> SpinLockGuard<'_, T> {
        let _raw_guard = self.raw_lock.lock();
        SpinLockGuard {
            _raw_guard,
            data: unsafe { &mut *self.data.get() },
        }
    }

    #[allow(dead_code)]
    pub fn is_locked(&self) -> bool {
        self.raw_lock.is_locked()
    }
}

pub struct SpinLockGuard<'a, T> {
    _raw_guard: RawSpinLockGuard<'a>,
    data: &'a mut T,
}

impl<T> core::ops::Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T> core::ops::DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

unsafe impl<T: Send> Send for SpinLock<T> {}
unsafe impl<T: Send> Sync for SpinLock<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kassert, test_case};

    // 基本功能：获取锁、修改数据、释放后检查值与锁状态
    test_case!(test_spinlock_basic, {
        println!("Testing: test_spinlock_basic");
        let lock = SpinLock::new(0usize);

        // 初始应未锁定
        kassert!(!lock.is_locked());

        unsafe {
            // 获取锁并修改数据
            {
                let mut guard = lock.lock();
                kassert!(lock.is_locked());
                *guard = 42;
                kassert!(*guard == 42);
            } // guard 离开作用域，释放锁
        }

        // 释放后应恢复为未锁定
        kassert!(!lock.is_locked());
    });

    // 检查释放后能再次加锁（避免在同一线程内重入）
    test_case!(test_spinlock_relock_after_drop, {
        println!("Testing: test_spinlock_relock_after_drop");
        let lock = SpinLock::new(1usize);

        unsafe {
            {
                let mut g1 = lock.lock();
                *g1 += 1;
                kassert!(*g1 == 2);
                // g1 在此作用域结束并释放锁
            }

            // 释放后，应该能再次获取锁
            let mut g2 = lock.lock();
            *g2 += 1;
            kassert!(*g2 == 3);
        }

        kassert!(!lock.is_locked());
    });
}
