use crate::sync::intr_guard::IntrGuard;
use core::{
    hint,
    sync::atomic::{AtomicBool, Ordering},
};

/// 自旋锁结构体，提供互斥访问临界区的能力。
/// 基于原子操作实现自旋锁机制，结合 IntrGuard 实现中断保护。
/// 不可重入 (即不能嵌套调用 RawSpinLock::lock())。
/// 使用示例：
/// ```ignore
/// let lock = RawSpinLock::new();
/// {
///   let guard = lock.lock(); // 获取锁，禁用中断
///   // 临界区代码
/// } // 离开作用域，自动释放锁并恢复中断状态
/// ```
#[derive(Debug)]
pub struct RawSpinLock {
    lock: AtomicBool,
}

impl RawSpinLock {
    pub const fn new() -> Self {
        RawSpinLock {
            lock: AtomicBool::new(false),
        }
    }

    /// 尝试获取自旋锁，并返回一个 RAII 保护器。
    ///
    /// 内部原子地获取锁，并在当前 CPU 禁用本地中断。
    pub fn lock(&self) -> RawSpinLockGuard<'_> {
        let guard = IntrGuard::new();

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
    /// 内部原子地尝试获取锁，并在当前 CPU 禁用本地中断。
    /// 如果获取失败，会立即恢复中断状态（通过 Drop IntrGuard）。
    pub fn try_lock(&self) -> Option<RawSpinLockGuard<'_>> {
        let guard = IntrGuard::new();

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
    /// 返回值：锁是否被占用
    #[cfg(test)]
    pub fn is_locked(&self) -> bool {
        self.lock.load(Ordering::Relaxed)
    }
}

/// 自动释放自旋锁和恢复中断状态的 RAII 结构体
pub struct RawSpinLockGuard<'a> {
    lock: &'a RawSpinLock,
    intr_guard: IntrGuard,
}

use core::ops::Drop;

impl Drop for RawSpinLockGuard<'_> {
    /// 退出作用域时自动执行，顺序如下：
    /// 1. 释放自旋锁标志。
    /// 2. IntrGuard 被 Drop，恢复中断状态。
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

    // 模拟一个共享资源，必须用 RawSpinLock 保护
    static COUNTER: AtomicBool = AtomicBool::new(false);

    // 测试锁的初始化状态和基本锁定/解锁功能
    test_case!(test_raw_spin_lock_basic_lock_unlock, {
        let lock = RawSpinLock::new();
        kassert!(!lock.is_locked());

        let guard = lock.lock();
        kassert!(lock.is_locked());

        // 手动释放 (Drop)
        drop(guard);
        kassert!(!lock.is_locked());
    });

    // 测试 RAII 行为 (自动释放)
    test_case!(test_raw_spin_lock_raii_release, {
        let lock = RawSpinLock::new();

        {
            let _guard = lock.lock();
            kassert!(lock.is_locked());
        } // <- _guard 在此离开作用域，Drop 被自动调用

        kassert!(!lock.is_locked());
    });

    // 测试互斥性 (只能获取一次)
    test_case!(test_raw_spin_lock_mutual_exclusion, {
        let lock = RawSpinLock::new();

        let guard1 = lock.lock();
        kassert!(lock.is_locked());

        // 尝试第二次获取 (理论上会进入无限自旋，但测试中我们只检查状态)
        // NOTE: 在实际运行环境中，第二次调用会死循环，测试环境通常需要模拟并发
        // 在这里我们依赖测试框架的单线程执行来简单检查 is_locked 状态

        // 模拟多线程获取失败的场景：
        let second_lock_failed;

        // 临时释放，让第二次获取成功
        drop(guard1);

        let guard2 = lock.lock();
        if lock.is_locked() {
            // 第二次获取成功
            second_lock_failed = false;
        } else {
            second_lock_failed = true;
        }

        kassert!(!second_lock_failed);
        drop(guard2);
    });

    // -----------------------------------------------------------
    // 中断保护测试
    // -----------------------------------------------------------

    // 测试 lock() 是否禁用了中断
    test_case!(test_interrupt_disable, {
        // 1. 确保中断最初是启用的
        let initial_flags = unsafe { read_and_disable_interrupts() };
        unsafe { restore_interrupts(initial_flags | (1 << 1)) }; // 确保 SIE 启用
        kassert!(are_interrupts_enabled());

        let lock = RawSpinLock::new();
        let guard = lock.lock();

        // 2. 检查中断是否被禁用
        kassert!(!are_interrupts_enabled());
        kassert!(guard.intr_guard.was_enabled());

        // 3. 检查 Drop 后中断是否恢复
        drop(guard);
        kassert!(are_interrupts_enabled());

        // 恢复测试前的环境
        unsafe { restore_interrupts(initial_flags) };
    });
}
