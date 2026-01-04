//! 读写锁实现
//!
//! 允许多个读者同时访问或单个写者独占访问。
//! 使用单个 AtomicUsize 编码状态：[WRITER(1bit)][READERS(31bits)]。
//!
//! # 不变量
//! - WRITER_BIT=1 时，READERS 必须为 0（写者独占）
//! - WRITER_BIT=0 时，READERS 可以 > 0（多读者共享）
//! - 读者数量不超过 READER_MASK (2^31-1)
//!
//! # 已知限制
//! - 读者溢出：超过 2^31-1 个读者时会 panic（实际不可能发生）
//! - 写者饥饿：连续读者可能饿死写者（内核场景可接受）
//! - 不支持锁升级/降级：尝试升级会死锁

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
pub struct RwLock<T> {
    state: AtomicUsize,
    data: UnsafeCell<T>,
}

/// 读锁的 RAII 保护器
pub struct RwLockReadGuard<'a, T> {
    lock: &'a RwLock<T>,
    intr_guard: IntrGuard,
}

/// 写锁的 RAII 保护器
pub struct RwLockWriteGuard<'a, T> {
    lock: &'a RwLock<T>,
    intr_guard: IntrGuard,
}

impl<T> RwLock<T> {
    /// 创建新的读写锁
    ///
    /// - data: 被保护的数据
    ///
    /// # 返回值
    /// - `RwLock<T>`: 新创建的读写锁
    pub const fn new(data: T) -> Self {
        RwLock {
            state: AtomicUsize::new(0),
            data: UnsafeCell::new(data),
        }
    }

    /// 获取读锁，返回 RAII 保护器
    ///
    /// 允许多个读者同时持有锁。如果有写者持有锁，则自旋等待。
    /// 自动禁用中断，离开作用域时恢复。
    ///
    /// # 返回值
    /// - `RwLockReadGuard`: RAII 保护器，提供共享数据访问
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        let intr_guard = IntrGuard::new();

        loop {
            let state = self.state.load(Ordering::Relaxed);

            // 检查是否有写者
            if state & WRITER_BIT != 0 {
                hint::spin_loop();
                continue;
            }

            // 检查读者数量是否溢出
            if (state & READER_MASK) == READER_MASK {
                panic!("RwLock: 读者数量溢出");
            }

            // 尝试增加读者计数
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

    /// 获取写锁，返回 RAII 保护器
    ///
    /// 独占访问，等待所有读者和写者退出。
    /// 自动禁用中断，离开作用域时恢复。
    ///
    /// # 返回值
    /// - `RwLockWriteGuard`: RAII 保护器，提供独占数据访问
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        let intr_guard = IntrGuard::new();

        // 等待直到可以设置写者标志
        loop {
            // 先检查 state 是否为 0，减少 CAS 失败时的总线争用
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

    /// 尝试获取读锁，如果成功则返回 RAII 保护器，否则返回 None
    ///
    /// 非阻塞版本，如果当前有写者则立即返回 None。
    ///
    /// # 返回值
    /// - `Some(RwLockReadGuard)`: 成功获取读锁
    /// - `None`: 有写者持有锁
    pub fn try_read(&self) -> Option<RwLockReadGuard<'_, T>> {
        let intr_guard = IntrGuard::new();

        let state = self.state.load(Ordering::Relaxed);

        // 检查是否有写者
        if state & WRITER_BIT != 0 {
            return None;
        }

        // 检查读者数量是否溢出
        if (state & READER_MASK) == READER_MASK {
            return None;
        }

        // 尝试增加读者计数
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

    /// 尝试获取写锁，如果成功则返回 RAII 保护器，否则返回 None
    ///
    /// 非阻塞版本，如果当前有读者或写者则立即返回 None。
    ///
    /// # 返回值
    /// - `Some(RwLockWriteGuard)`: 成功获取写锁
    /// - `None`: 有读者或写者持有锁
    pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>> {
        let intr_guard = IntrGuard::new();

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

    /// 获取当前读者数量（仅用于测试）
    ///
    /// # 返回值
    /// - 当前读者数量
    #[cfg(test)]
    pub fn reader_count(&self) -> usize {
        self.state.load(Ordering::Relaxed) & READER_MASK
    }

    /// 检查是否有写者持有锁（仅用于测试）
    ///
    /// # 返回值
    /// - `true`: 有写者持有锁
    /// - `false`: 无写者
    #[cfg(test)]
    pub fn is_write_locked(&self) -> bool {
        self.state.load(Ordering::Relaxed) & WRITER_BIT != 0
    }
}

impl<T> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: 持有读锁，保证无写者
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.state.fetch_sub(1, Ordering::Release);
    }
}

impl<T> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: 持有写锁，保证独占访问
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: 持有写锁，保证独占访问
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.state.store(0, Ordering::Release);
    }
}

// SAFETY: RwLock 可以在线程间传递，只要 T 是 Send
unsafe impl<T: Send> Send for RwLock<T> {}

// SAFETY: RwLock 可以在线程间共享，只要 T 是 Send + Sync
unsafe impl<T: Send + Sync> Sync for RwLock<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{arch::intr::are_interrupts_enabled, kassert, test_case};

    test_case!(test_rwlock_read_basic, {
        let lock = RwLock::new(42);

        let guard = lock.read();
        kassert!(*guard == 42);
        kassert!(lock.reader_count() == 1);
        kassert!(!lock.is_write_locked());

        drop(guard);
        kassert!(lock.reader_count() == 0);
    });

    test_case!(test_rwlock_write_basic, {
        let lock = RwLock::new(100);

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
        let lock = RwLock::new(42);

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
        let lock = RwLock::new(0);

        let guard = lock.write();
        kassert!(lock.is_write_locked());

        // 尝试获取读锁应该失败
        let read_guard = lock.try_read();
        kassert!(read_guard.is_none());

        drop(guard);

        // 释放写锁后应该可以获取读锁
        let read_guard = lock.try_read();
        kassert!(read_guard.is_some());
    });

    test_case!(test_rwlock_interrupt_disable, {
        let lock = RwLock::new(0);

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
        let lock = RwLock::new(42);

        if let Some(guard) = lock.try_read() {
            kassert!(*guard == 42);
        } else {
            kassert!(false); // try_read 应该成功
        }
    });

    test_case!(test_rwlock_try_write, {
        let lock = RwLock::new(100);

        let guard = lock.try_write();
        kassert!(guard.is_some());

        // 持有写锁时，再次 try_write 应该失败
        let guard2 = lock.try_write();
        kassert!(guard2.is_none());

        drop(guard);

        // 释放后应该可以获取
        let guard3 = lock.try_write();
        kassert!(guard3.is_some());
    });
}
