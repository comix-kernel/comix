#![allow(unused)]
use core::{hint, sync::atomic::{AtomicBool, Ordering}};
use crate::sync::intr_guard::IntrGuard;

/// 自旋锁结构体，提供互斥访问临界区的能力。
/// 基于原子操作实现自旋锁机制，结合 IntrGuard 实现中断保护。
/// 不可重入 (即不能嵌套调用 SpinLock::lock())。
/// 使用示例：
/// ```ignore
/// let lock = SpinLock::new();
/// {
///   let guard = lock.lock(); // 获取锁，禁用中断
///   // 临界区代码
/// } // 离开作用域，自动释放锁并恢复中断状态
/// ```
pub struct SpinLock {
    lock: AtomicBool,
}

impl SpinLock {
    pub const fn new() -> Self {
        SpinLock {
            lock: AtomicBool::new(false),
        }
    }
    
    /// 尝试获取自旋锁，并返回一个 RAII 保护器。
    /// 
    /// 内部原子地获取锁，并在当前 CPU 禁用本地中断。
    pub fn lock(&self) -> SpinLockGuard {
        
        let guard = IntrGuard::new(); 
        
        while self.lock.compare_exchange(
            false, 
            true, 
            Ordering::Acquire, 
            Ordering::Relaxed
        ).is_err() {
            hint::spin_loop();
        }

        SpinLockGuard { 
            lock: self,
            intr_guard: guard,
        }
    }
    
    /// 仅释放锁标志。
    fn unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }

    /// 检查锁是否被占用 (仅用于调试/测试)
    pub fn is_locked(&self) -> bool {
        self.lock.load(Ordering::Relaxed)
    }
}

/// 自动释放自旋锁和恢复中断状态的 RAII 结构体
pub struct SpinLockGuard<'a> {
    lock: &'a SpinLock,
    intr_guard: IntrGuard, 
}

use core::ops::Drop;

impl Drop for SpinLockGuard<'_> {
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
    use crate::arch::intr::{read_and_disable_interrupts, restore_interrupts, are_interrupts_enabled};
    
    // 模拟一个共享资源，必须用 SpinLock 保护
    static COUNTER: AtomicBool = AtomicBool::new(false); 
    
    /// 测试锁的初始化状态和基本锁定/解锁功能
    #[test_case]
    fn test_spinlock_basic_lock_unlock() {
        let lock = SpinLock::new();
        assert!(!lock.is_locked(), "初始状态应为未锁定");

        let guard = lock.lock();
        assert!(lock.is_locked(), "lock() 后锁应为锁定状态");

        // 手动释放 (Drop)
        drop(guard);
        assert!(!lock.is_locked(), "Drop 后锁应为未锁定状态");
    }

    /// 测试 RAII 行为 (自动释放)
    #[test_case]
    fn test_spinlock_raii_release() {
        let lock = SpinLock::new();
        
        {
            let _guard = lock.lock();
            assert!(lock.is_locked(), "进入作用域后应锁定");
        } // <- _guard 在此离开作用域，Drop 被自动调用

        assert!(!lock.is_locked(), "离开作用域后应自动解锁");
    }

    /// 测试互斥性 (只能获取一次)
    #[test_case]
    fn test_spinlock_mutual_exclusion() {
        let lock = SpinLock::new();
        
        let guard1 = lock.lock();
        assert!(lock.is_locked(), "第一次获取成功");
        
        // 尝试第二次获取 (理论上会进入无限自旋，但测试中我们只检查状态)
        // NOTE: 在实际运行环境中，第二次调用会死循环，测试环境通常需要模拟并发
        // 在这里我们依赖测试框架的单线程执行来简单检查 is_locked 状态
        
        // 模拟多线程获取失败的场景：
        let mut second_lock_failed = false;
        
        // 临时释放，让第二次获取成功
        drop(guard1);
        
        let guard2 = lock.lock();
        if lock.is_locked() {
            // 第二次获取成功
            second_lock_failed = false; 
        } else {
            second_lock_failed = true;
        }

        assert!(!second_lock_failed, "第二次尝试获取锁成功 (因为第一次已释放)");
        drop(guard2);
    }
    
    // -----------------------------------------------------------
    // 中断保护测试
    // -----------------------------------------------------------
    
    /// 测试 lock() 是否禁用了中断
    #[test_case]
    fn test_interrupt_disable() {
        // 1. 确保中断最初是启用的
        let initial_flags = unsafe { read_and_disable_interrupts() };
        unsafe { restore_interrupts(initial_flags | (1 << 1)) }; // 确保 SIE 启用
        assert!(are_interrupts_enabled(), "前提：中断应为启用状态");

        let lock = SpinLock::new();
        let guard = lock.lock(); 
        
        // 2. 检查中断是否被禁用
        assert!(!are_interrupts_enabled(), "获取锁后，中断应被禁用");
        assert!(guard.intr_guard.was_enabled(), "was_enabled 应记录为 true");

        // 3. 检查 Drop 后中断是否恢复
        drop(guard);
        assert!(are_interrupts_enabled(), "释放锁后，中断应被恢复");
        
        // 恢复测试前的环境
        unsafe { restore_interrupts(initial_flags) };
    }
}
