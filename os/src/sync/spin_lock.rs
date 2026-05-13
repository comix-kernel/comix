//! 自旋锁
//!
//! 提供对数据的互斥访问的自旋锁结构体。
//! 内部包含一个 RawSpinLock 和一个 UnsafeCell 用于存储数据。
//!
//! 注意：SpinLock 不是可重入的。当持有锁时，尝试再次获取锁将导致死锁。
//! 此外，SpinLock 通过禁用中断来保护临界区，因此在持有锁时应避免长时间运行的操作。
//!
//! # 泛型参数
//!
//! * `T` - 被保护的数据类型
//! * `CPU` - 实现 `CpuOps` 的类型，默认使用 `ArchImpl`

use core::cell::UnsafeCell;

use crate::arch::ArchImpl;
use crate::hal::CpuOps;
use crate::sync::raw_spin_lock::{RawSpinLock, RawSpinLockGuard};

/// 提供对数据的互斥访问的自旋锁结构体。
///
/// 使用示例：
/// ```ignore
/// let lock = SpinLock::new(0);
/// {
///     let mut guard = lock.lock();
///     *guard += 1;
/// }
/// ```
pub struct SpinLock<T, CPU: CpuOps = ArchImpl> {
    raw_lock: RawSpinLock<CPU>,
    data: UnsafeCell<T>,
}

impl<T: core::fmt::Debug, CPU: CpuOps> core::fmt::Debug for SpinLock<T, CPU> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SpinLock")
            .field("data", unsafe { &*self.data.get() })
            .finish()
    }
}

impl<T, CPU: CpuOps> SpinLock<T, CPU> {
    /// 创建一个新的 SpinLock 实例，初始化内部数据。
    pub const fn new(data: T) -> Self {
        SpinLock {
            raw_lock: RawSpinLock::new(),
            data: UnsafeCell::new(data),
        }
    }

    /// 获取自旋锁，并返回一个 RAII 保护器，用于访问和修改内部数据。
    pub fn lock(&self) -> SpinLockGuard<'_, T, CPU> {
        let _raw_guard = self.raw_lock.lock();
        SpinLockGuard {
            _raw_guard,
            data: unsafe { &mut *self.data.get() },
        }
    }

    /// 尝试获取自旋锁，如果成功则返回 RAII 保护器，否则返回 None。
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T, CPU>> {
        self.raw_lock.try_lock().map(|_raw_guard| SpinLockGuard {
            _raw_guard,
            data: unsafe { &mut *self.data.get() },
        })
    }

    /// 检查锁是否被占用 (仅用于调试/测试)
    #[cfg(test)]
    pub fn is_locked(&self) -> bool {
        self.raw_lock.is_locked()
    }
}

/// SpinLock 的 RAII 保护器，提供对锁定数据的访问。
pub struct SpinLockGuard<'a, T, CPU: CpuOps = ArchImpl> {
    _raw_guard: RawSpinLockGuard<'a, CPU>,
    data: &'a mut T,
}

impl<T, CPU: CpuOps> core::ops::Deref for SpinLockGuard<'_, T, CPU> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T, CPU: CpuOps> core::ops::DerefMut for SpinLockGuard<'_, T, CPU> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

// Safety: SpinLock 可以在线程间安全共享
unsafe impl<T: Send, CPU: CpuOps> Send for SpinLock<T, CPU> {}
unsafe impl<T: Send, CPU: CpuOps> Sync for SpinLock<T, CPU> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kassert, println, test_case};

    test_case!(test_spinlock_basic, {
        println!("Testing: test_spinlock_basic");
        let lock = SpinLock::<usize>::new(0);

        kassert!(!lock.is_locked());

        {
            let mut guard = lock.lock();
            kassert!(lock.is_locked());
            *guard = 42;
            kassert!(*guard == 42);
        }

        kassert!(!lock.is_locked());
    });

    test_case!(test_spinlock_relock_after_drop, {
        println!("Testing: test_spinlock_relock_after_drop");
        let lock = SpinLock::<usize>::new(1);

        {
            let mut g1 = lock.lock();
            *g1 += 1;
            kassert!(*g1 == 2);
        }

        {
            let mut g2 = lock.lock();
            *g2 += 1;
            kassert!(*g2 == 3);
        }
        kassert!(!lock.is_locked());
    });
}
