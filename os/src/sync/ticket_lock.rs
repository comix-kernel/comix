//! 票号锁实现
//!
//! 提供公平的 FIFO 顺序锁获取机制，避免饥饿问题。
//! 使用两个原子计数器：next_ticket（下一个票号）和 serving_ticket（当前服务票号）。
//! 每个线程获取唯一票号，按顺序等待服务。
//!
//! # 不变量
//! - serving_ticket <= next_ticket
//! - 持有锁时 serving_ticket == 某个线程的 ticket
//! - 释放锁时 serving_ticket 递增 1
//!
//! # 已知限制
//! - 票号溢出：next_ticket 在 usize::MAX 时回绕，可能导致死锁（实际不太可能发生）

use crate::sync::intr_guard::IntrGuard;
use core::{
    cell::UnsafeCell,
    hint,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicUsize, Ordering},
};

/// 票号锁，提供公平的 FIFO 顺序获取
pub struct TicketLock<T> {
    next_ticket: AtomicUsize,
    serving_ticket: AtomicUsize,
    data: UnsafeCell<T>,
}

/// 票号锁的 RAII 保护器
pub struct TicketLockGuard<'a, T> {
    lock: &'a TicketLock<T>,
    intr_guard: IntrGuard,
}

impl<T> TicketLock<T> {
    /// 创建新的票号锁
    ///
    /// - data: 被保护的数据
    ///
    /// # 返回值
    /// - `TicketLock<T>`: 新创建的票号锁
    pub const fn new(data: T) -> Self {
        TicketLock {
            next_ticket: AtomicUsize::new(0),
            serving_ticket: AtomicUsize::new(0),
            data: UnsafeCell::new(data),
        }
    }

    /// 获取锁，返回 RAII 保护器
    ///
    /// 自动禁用中断，按 FIFO 顺序获取锁。
    /// 如果锁被占用，则自旋等待直到轮到当前线程。
    ///
    /// # 返回值
    /// - `TicketLockGuard`: RAII 保护器，提供数据访问
    pub fn lock(&self) -> TicketLockGuard<'_, T> {
        let intr_guard = IntrGuard::new();

        // 获取票号
        let my_ticket = self.next_ticket.fetch_add(1, Ordering::Relaxed);

        // 等待轮到自己
        while self.serving_ticket.load(Ordering::Acquire) != my_ticket {
            hint::spin_loop();
        }

        TicketLockGuard {
            lock: self,
            intr_guard,
        }
    }

    /// 尝试获取锁，如果成功则返回 RAII 保护器，否则返回 None
    ///
    /// 非阻塞版本，如果当前无法立即获取锁则返回 None。
    ///
    /// # 返回值
    /// - `Some(TicketLockGuard)`: 成功获取锁
    /// - `None`: 锁被占用
    pub fn try_lock(&self) -> Option<TicketLockGuard<'_, T>> {
        let intr_guard = IntrGuard::new();

        let serving = self.serving_ticket.load(Ordering::Relaxed);
        let next = self.next_ticket.load(Ordering::Relaxed);

        // 只有在没有人等待时才能获取
        if serving == next {
            // 尝试获取票号
            if self
                .next_ticket
                .compare_exchange(next, next + 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return Some(TicketLockGuard {
                    lock: self,
                    intr_guard,
                });
            }
        }

        None
    }

    /// 检查锁是否被占用（仅用于测试）
    ///
    /// # 返回值
    /// - `true`: 锁被占用
    /// - `false`: 锁空闲
    #[cfg(test)]
    pub fn is_locked(&self) -> bool {
        self.next_ticket.load(Ordering::Relaxed) != self.serving_ticket.load(Ordering::Relaxed)
    }
}

impl<T> Deref for TicketLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: 持有锁，保证独占访问
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for TicketLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: 持有锁，保证独占访问
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for TicketLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.serving_ticket.fetch_add(1, Ordering::Release);
    }
}

// SAFETY: TicketLock 可以在线程间传递，只要 T 是 Send
unsafe impl<T: Send> Send for TicketLock<T> {}

// SAFETY: TicketLock 可以在线程间共享，只要 T 是 Send + Sync
unsafe impl<T: Send + Sync> Sync for TicketLock<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{arch::intr::are_interrupts_enabled, kassert, test_case};

    test_case!(test_ticket_lock_basic, {
        let lock = TicketLock::new(42);
        kassert!(!lock.is_locked());

        let guard = lock.lock();
        kassert!(lock.is_locked());
        kassert!(*guard == 42);

        drop(guard);
        kassert!(!lock.is_locked());
    });

    test_case!(test_ticket_lock_raii, {
        let lock = TicketLock::new(100);

        {
            let mut guard = lock.lock();
            kassert!(lock.is_locked());
            *guard = 200;
        }

        kassert!(!lock.is_locked());

        let guard = lock.lock();
        kassert!(*guard == 200);
    });

    test_case!(test_ticket_lock_fairness, {
        let lock = TicketLock::new(0);

        // 获取第一个票号
        let guard1 = lock.lock();
        kassert!(lock.next_ticket.load(Ordering::Relaxed) == 1);
        kassert!(lock.serving_ticket.load(Ordering::Relaxed) == 0);

        drop(guard1);

        // 获取第二个票号
        let guard2 = lock.lock();
        kassert!(lock.next_ticket.load(Ordering::Relaxed) == 2);
        kassert!(lock.serving_ticket.load(Ordering::Relaxed) == 1);

        drop(guard2);
    });

    test_case!(test_ticket_lock_interrupt_disable, {
        let lock = TicketLock::new(0);

        let guard = lock.lock();
        kassert!(!are_interrupts_enabled());

        drop(guard);
        kassert!(are_interrupts_enabled());
    });

    test_case!(test_ticket_lock_try_lock, {
        let lock = TicketLock::new(42);

        // 第一次 try_lock 应该成功
        let guard = lock.try_lock();
        kassert!(guard.is_some());
        kassert!(lock.is_locked());

        // 第二次 try_lock 应该失败（锁已被占用）
        let guard2 = lock.try_lock();
        kassert!(guard2.is_none());

        drop(guard);

        // 释放后再次 try_lock 应该成功
        let guard3 = lock.try_lock();
        kassert!(guard3.is_some());
    });
}
