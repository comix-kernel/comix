//! 无 Guard 的底层自旋锁，专门为全局分配器集成设计
//!
//! 实现 `lock_api::RawMutex` trait，用于 `talc` 分配器的 `Talck` 类型。
//!
//! # 与 `RawSpinLock` 的主要区别
//!
//! - 实现 `lock_api::RawMutex` trait
//! - `lock()` 方法不返回 Guard
//! - 使用 `AtomicUsize` 在内部存储中断状态
//! - `unlock()` 操作恢复中断状态
//!
//! # 泛型参数
//!
//! * `CPU` - 实现 `CpuOps` 的类型，默认使用 `ArchImpl`

use crate::arch::ArchImpl;
use crate::hal::CpuOps;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// 自旋锁结构体，不返回 Guard，集成了中断状态保存与恢复功能。
pub struct RawSpinLockWithoutGuard<CPU: CpuOps = ArchImpl> {
    locked: AtomicBool,
    saved_intr_flags: AtomicUsize,
    _marker: PhantomData<CPU>,
}

impl<CPU: CpuOps> RawSpinLockWithoutGuard<CPU> {
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
            saved_intr_flags: AtomicUsize::new(0),
            _marker: PhantomData,
        }
    }
}

unsafe impl<CPU: CpuOps> lock_api::RawMutex for RawSpinLockWithoutGuard<CPU> {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self::new();

    type GuardMarker = lock_api::GuardNoSend;

    fn lock(&self) {
        let flags = CPU::disable_interrupts();

        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }

        self.saved_intr_flags.store(flags, Ordering::Release);
    }

    fn try_lock(&self) -> bool {
        let flags = CPU::disable_interrupts();

        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            self.saved_intr_flags.store(flags, Ordering::Release);
            true
        } else {
            CPU::restore_interrupt_state(flags);
            false
        }
    }

    unsafe fn unlock(&self) {
        let flags = self.saved_intr_flags.load(Ordering::Acquire);
        self.locked.store(false, Ordering::Release);
        CPU::restore_interrupt_state(flags);
    }
}

unsafe impl<CPU: CpuOps> Send for RawSpinLockWithoutGuard<CPU> {}
unsafe impl<CPU: CpuOps> Sync for RawSpinLockWithoutGuard<CPU> {}

#[cfg(test)]
mod tests {
    use lock_api::RawMutex;

    use super::*;
    use crate::{kassert, test_case};

    test_case!(test_mutex_wrapper_guard_basic, {
        let m = lock_api::Mutex::<RawSpinLockWithoutGuard, usize>::new(0);

        {
            let mut g = m.lock();
            *g = 42;
        }

        {
            let g = m.lock();
            kassert!(*g == 42);
        }
    });

    test_case!(test_try_lock_and_unlock_roundtrip, {
        let raw = RawSpinLockWithoutGuard::<ArchImpl>::new();

        let ok = raw.try_lock();
        kassert!(ok);

        let fail = raw.try_lock();
        kassert!(!fail);

        unsafe {
            raw.unlock();
        }

        let ok2 = raw.try_lock();
        kassert!(ok2);

        unsafe {
            raw.unlock();
        }
    });

    test_case!(test_lock_then_unlock, {
        let raw = RawSpinLockWithoutGuard::<ArchImpl>::new();

        raw.lock();

        let fail = raw.try_lock();
        kassert!(!fail);

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
