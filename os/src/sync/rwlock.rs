//! 读写锁实现
//!
//! 允许多个读者同时访问或单个写者独占访问。
//!
//! # 泛型参数
//!
//! * `T` - 被保护的数据类型
//! * `CPU` - 实现 `CpuOps` 的类型，默认使用 `ArchImpl`

use crate::arch::ArchImpl;
use crate::arch::CpuOps;
use crate::sync::intr_guard::IntrGuard;
use core::{
    cell::UnsafeCell,
    hint,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicUsize, Ordering},
};

const WRITER_BIT: usize = 1 << 31;
const READER_MASK: usize = WRITER_BIT - 1;

/// 读写锁，允许多个读者或单个写者
pub struct RwLock<T, CPU: CpuOps = ArchImpl> {
    state: AtomicUsize,
    data: UnsafeCell<T>,
    _marker: core::marker::PhantomData<CPU>,
}

/// 读锁的 RAII 保护器
pub struct RwLockReadGuard<'a, T, CPU: CpuOps = ArchImpl> {
    lock: &'a RwLock<T, CPU>,
    intr_guard: IntrGuard<CPU>,
}

/// 写锁的 RAII 保护器
pub struct RwLockWriteGuard<'a, T, CPU: CpuOps = ArchImpl> {
    lock: &'a RwLock<T, CPU>,
    intr_guard: IntrGuard<CPU>,
}

impl<T, CPU: CpuOps> RwLock<T, CPU> {
    pub const fn new(data: T) -> Self {
        RwLock {
            state: AtomicUsize::new(0),
            data: UnsafeCell::new(data),
            _marker: core::marker::PhantomData,
        }
    }

    pub fn read(&self) -> RwLockReadGuard<'_, T, CPU> {
        let intr_guard = IntrGuard::<CPU>::new();

        loop {
            let state = self.state.load(Ordering::Relaxed);

            if state & WRITER_BIT != 0 {
                hint::spin_loop();
                continue;
            }

            if (state & READER_MASK) == READER_MASK {
                panic!("RwLock: reader count overflow");
            }

            if self
                .state
                .compare_exchange_weak(state, state + 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return RwLockReadGuard {
                    lock: self,
                    intr_guard,
                };
            }
        }
    }

    pub fn write(&self) -> RwLockWriteGuard<'_, T, CPU> {
        let intr_guard = IntrGuard::<CPU>::new();

        loop {
            if self.state.load(Ordering::Relaxed) == 0
                && self
                    .state
                    .compare_exchange_weak(0, WRITER_BIT, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
            {
                return RwLockWriteGuard {
                    lock: self,
                    intr_guard,
                };
            }
            hint::spin_loop();
        }
    }

    pub fn try_read(&self) -> Option<RwLockReadGuard<'_, T, CPU>> {
        let intr_guard = IntrGuard::<CPU>::new();

        let state = self.state.load(Ordering::Relaxed);

        if state & WRITER_BIT != 0 {
            return None;
        }

        if (state & READER_MASK) == READER_MASK {
            return None;
        }

        if self
            .state
            .compare_exchange(state, state + 1, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(RwLockReadGuard {
                lock: self,
                intr_guard,
            })
        } else {
            None
        }
    }

    pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, T, CPU>> {
        let intr_guard = IntrGuard::<CPU>::new();

        if self
            .state
            .compare_exchange(0, WRITER_BIT, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(RwLockWriteGuard {
                lock: self,
                intr_guard,
            })
        } else {
            None
        }
    }

    #[cfg(test)]
    pub fn reader_count(&self) -> usize {
        self.state.load(Ordering::Relaxed) & READER_MASK
    }

    #[cfg(test)]
    pub fn is_write_locked(&self) -> bool {
        self.state.load(Ordering::Relaxed) & WRITER_BIT != 0
    }
}

impl<T, CPU: CpuOps> Deref for RwLockReadGuard<'_, T, CPU> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T, CPU: CpuOps> Drop for RwLockReadGuard<'_, T, CPU> {
    fn drop(&mut self) {
        self.lock.state.fetch_sub(1, Ordering::Release);
    }
}

impl<T, CPU: CpuOps> Deref for RwLockWriteGuard<'_, T, CPU> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T, CPU: CpuOps> DerefMut for RwLockWriteGuard<'_, T, CPU> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T, CPU: CpuOps> Drop for RwLockWriteGuard<'_, T, CPU> {
    fn drop(&mut self) {
        self.lock.state.store(0, Ordering::Release);
    }
}

unsafe impl<T: Send, CPU: CpuOps> Send for RwLock<T, CPU> {}
unsafe impl<T: Send + Sync, CPU: CpuOps> Sync for RwLock<T, CPU> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{arch::intr::are_interrupts_enabled, kassert, test_case};

    test_case!(test_rwlock_read_basic, {
        let lock: RwLock<i32> = RwLock::new(42);

        let guard = lock.read();
        kassert!(*guard == 42);
        kassert!(lock.reader_count() == 1);
        kassert!(!lock.is_write_locked());

        drop(guard);
        kassert!(lock.reader_count() == 0);
    });

    test_case!(test_rwlock_write_basic, {
        let lock: RwLock<i32> = RwLock::new(100);

        let mut guard = lock.write();
        kassert!(*guard == 100);
        kassert!(lock.is_write_locked());
        kassert!(lock.reader_count() == 0);

        *guard = 200;
        drop(guard);

        kassert!(!lock.is_write_locked());

        let guard = lock.read();
        kassert!(*guard == 200);
    });

    test_case!(test_rwlock_multiple_readers, {
        let lock: RwLock<i32> = RwLock::new(42);

        let guard1 = lock.read();
        kassert!(lock.reader_count() == 1);

        let guard2 = lock.read();
        kassert!(lock.reader_count() == 2);

        let guard3 = lock.read();
        kassert!(lock.reader_count() == 3);

        kassert!(*guard1 == 42);
        kassert!(*guard2 == 42);
        kassert!(*guard3 == 42);

        drop(guard1);
        kassert!(lock.reader_count() == 2);

        drop(guard2);
        kassert!(lock.reader_count() == 1);

        drop(guard3);
        kassert!(lock.reader_count() == 0);
    });

    test_case!(test_rwlock_writer_excludes_readers, {
        let lock: RwLock<i32> = RwLock::new(0);

        let guard = lock.write();
        kassert!(lock.is_write_locked());

        let read_guard = lock.try_read();
        kassert!(read_guard.is_none());

        drop(guard);

        let read_guard = lock.try_read();
        kassert!(read_guard.is_some());
    });

    test_case!(test_rwlock_interrupt_disable, {
        let lock: RwLock<i32> = RwLock::new(0);

        let guard = lock.read();
        kassert!(!are_interrupts_enabled());
        drop(guard);
        kassert!(are_interrupts_enabled());

        let guard = lock.write();
        kassert!(!are_interrupts_enabled());
        drop(guard);
        kassert!(are_interrupts_enabled());
    });

    test_case!(test_rwlock_try_read, {
        let lock: RwLock<i32> = RwLock::new(42);

        if let Some(guard) = lock.try_read() {
            kassert!(*guard == 42);
        } else {
            kassert!(false);
        }
    });

    test_case!(test_rwlock_try_write, {
        let lock: RwLock<i32> = RwLock::new(100);

        let guard = lock.try_write();
        kassert!(guard.is_some());

        let guard2 = lock.try_write();
        kassert!(guard2.is_none());

        drop(guard);

        let guard3 = lock.try_write();
        kassert!(guard3.is_some());
    });
}
