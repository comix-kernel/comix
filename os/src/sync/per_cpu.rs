//! Per-CPU 变量机制
//!
//! 允许每个 CPU 维护独立的数据副本，避免锁竞争。

use alloc::vec::Vec;
use core::cell::UnsafeCell;

/// Per-CPU 变量容器
///
/// 为每个 CPU 维护一个独立的 T 类型数据副本。
pub struct PerCpu<T> {
    data: Vec<UnsafeCell<T>>,
}

impl<T> PerCpu<T> {
    /// 创建 Per-CPU 变量
    ///
    /// - init: 初始化函数，为每个 CPU 创建一个数据副本
    ///
    /// # Panics
    ///
    /// 如果 NUM_CPU 未设置或为 0，会 panic
    pub fn new<F: Fn() -> T>(init: F) -> Self {
        let num_cpu = unsafe { crate::kernel::NUM_CPU };
        assert!(num_cpu > 0, "NUM_CPU must be set before creating PerCpu");

        let mut data = Vec::with_capacity(num_cpu);
        for _ in 0..num_cpu {
            data.push(UnsafeCell::new(init()));
        }
        PerCpu { data }
    }

    /// 获取当前 CPU 的数据（只读）
    ///
    /// # Safety
    ///
    /// 调用者必须确保：
    /// 1. 当前 CPU ID 有效（< NUM_CPU）
    /// 2. 访问期间抢占已禁用（防止任务迁移）
    #[inline]
    pub fn get(&self) -> &T {
        let cpu_id = crate::arch::kernel::cpu::cpu_id();
        // SAFETY: cpu_id 由 tp 寄存器获取，保证有效
        unsafe { &*self.data[cpu_id].get() }
    }

    /// 获取当前 CPU 的数据（可变）
    ///
    /// # Safety
    ///
    /// 调用者必须确保：
    /// 1. 当前 CPU ID 有效
    /// 2. 访问期间抢占已禁用
    /// 3. 没有其他引用指向同一数据
    #[inline]
    pub fn get_mut(&self) -> &mut T {
        let cpu_id = crate::arch::kernel::cpu::cpu_id();
        // SAFETY: 调用者保证独占访问
        unsafe { &mut *self.data[cpu_id].get() }
    }

    /// 获取指定 CPU 的数据（只读）
    ///
    /// 用于跨核访问，例如负载均衡时查看其他 CPU 的队列长度。
    #[inline]
    pub fn get_of(&self, cpu_id: usize) -> &T {
        assert!(cpu_id < self.data.len(), "Invalid CPU ID");
        // SAFETY: cpu_id 已检查有效性
        unsafe { &*self.data[cpu_id].get() }
    }
}

// SAFETY: PerCpu<T> 可以在线程间传递，因为每个 CPU 访问不同的数据
unsafe impl<T: Send> Send for PerCpu<T> {}
unsafe impl<T: Send> Sync for PerCpu<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kassert, test_case};
    use core::sync::atomic::{AtomicUsize, Ordering};

    test_case!(test_per_cpu_basic, {
        let per_cpu = PerCpu::new(|| AtomicUsize::new(0));
        let counter = per_cpu.get();
        kassert!(counter.load(Ordering::Relaxed) == 0);
        counter.fetch_add(1, Ordering::Relaxed);
        kassert!(counter.load(Ordering::Relaxed) == 1);
    });

    test_case!(test_per_cpu_get_of, {
        let per_cpu = PerCpu::new(|| AtomicUsize::new(42));
        let counter = per_cpu.get_of(0);
        kassert!(counter.load(Ordering::Relaxed) == 42);
        counter.fetch_add(1, Ordering::Relaxed);
        kassert!(per_cpu.get_of(0).load(Ordering::Relaxed) == 43);
    });

    test_case!(test_per_cpu_get_mut, {
        let per_cpu = PerCpu::new(|| 0usize);
        let value = per_cpu.get_mut();
        kassert!(*value == 0);
        *value = 100;
        kassert!(*per_cpu.get_mut() == 100);
    });
}
