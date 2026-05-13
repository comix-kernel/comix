//! 票号锁实现
//!
//! 提供公平的 FIFO 顺序锁获取机制，避免饥饿问题。
//!
//! # 泛型参数
//!
//! * `T` - 被保护的数据类型
//! * `CPU` - 实现 `CpuOps` 的类型，默认使用 `ArchImpl`

use crate::arch::ArchImpl;
use crate::hal::CpuOps;
use crate::sync::intr_guard::IntrGuard;
use core::{
    cell::UnsafeCell,
    hint,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicUsize, Ordering},
};

/// 票号锁，提供公平的 FIFO 顺序获取
pub struct TicketLock<T, CPU: CpuOps = ArchImpl> {
    next_ticket: AtomicUsize,
    serving_ticket: AtomicUsize,
    data: UnsafeCell<T>,
    _marker: core::marker::PhantomData<CPU>,
}

/// 票号锁的 RAII 保护器
pub struct TicketLockGuard<'a, T, CPU: CpuOps = ArchImpl> {
    lock: &'a TicketLock<T, CPU>,
    intr_guard: IntrGuard<CPU>,
}

impl<T, CPU: CpuOps> TicketLock<T, CPU> {
    pub const fn new(data: T) -> Self {
        TicketLock {
            next_ticket: AtomicUsize::new(0),
            serving_ticket: AtomicUsize::new(0),
            data: UnsafeCell::new(data),
            _marker: core::marker::PhantomData,
        }
    }

    pub fn lock(&self) -> TicketLockGuard<'_, T, CPU> {
        let intr_guard = IntrGuard::<CPU>::new();

        let my_ticket = self.next_ticket.fetch_add(1, Ordering::Relaxed);

        while self.serving_ticket.load(Ordering::Acquire) != my_ticket {
            hint::spin_loop();
        }

        TicketLockGuard {
            lock: self,
            intr_guard,
        }
    }

    pub fn try_lock(&self) -> Option<TicketLockGuard<'_, T, CPU>> {
        let intr_guard = IntrGuard::<CPU>::new();

        let serving = self.serving_ticket.load(Ordering::Relaxed);
        let next = self.next_ticket.load(Ordering::Relaxed);

        if serving == next {
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

    #[cfg(test)]
    pub fn is_locked(&self) -> bool {
        self.next_ticket.load(Ordering::Relaxed) != self.serving_ticket.load(Ordering::Relaxed)
    }
}

impl<T, CPU: CpuOps> Deref for TicketLockGuard<'_, T, CPU> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T, CPU: CpuOps> DerefMut for TicketLockGuard<'_, T, CPU> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T, CPU: CpuOps> Drop for TicketLockGuard<'_, T, CPU> {
    fn drop(&mut self) {
        self.lock.serving_ticket.fetch_add(1, Ordering::Release);
    }
}

unsafe impl<T: Send, CPU: CpuOps> Send for TicketLock<T, CPU> {}
unsafe impl<T: Send + Sync, CPU: CpuOps> Sync for TicketLock<T, CPU> {}

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

        let guard1 = lock.lock();
        kassert!(lock.next_ticket.load(Ordering::Relaxed) == 1);
        kassert!(lock.serving_ticket.load(Ordering::Relaxed) == 0);

        drop(guard1);

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

        let guard = lock.try_lock();
        kassert!(guard.is_some());
        kassert!(lock.is_locked());

        let guard2 = lock.try_lock();
        kassert!(guard2.is_none());

        drop(guard);

        let guard3 = lock.try_lock();
        kassert!(guard3.is_some());
    });
}
