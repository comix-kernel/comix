//! Per-CPU 变量机制
//!
//! 允许每个 CPU 维护独立的数据副本，避免锁竞争。
//!
//! # 泛型参数
//!
//! * `T` - 每个 CPU 存储的数据类型
//! * `CPU` - 实现 `CpuOps` 的类型，默认使用 `ArchImpl`

use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::marker::PhantomData;

use crate::arch::ArchImpl;
use crate::hal::CpuOps;

const CACHE_LINE_SIZE: usize = 64;

/// 缓存行对齐的包装结构
#[repr(align(64))]
struct CacheAligned<T>(UnsafeCell<T>);

impl<T> CacheAligned<T> {
    fn new(value: T) -> Self {
        CacheAligned(UnsafeCell::new(value))
    }

    fn get(&self) -> *mut T {
        self.0.get()
    }
}

/// Per-CPU 变量容器
///
/// 为每个 CPU 维护一个独立的 T 类型数据副本。
pub struct PerCpu<T, CPU: CpuOps = ArchImpl> {
    data: Vec<CacheAligned<T>>,
    _marker: PhantomData<CPU>,
}

impl<T, CPU: CpuOps> PerCpu<T, CPU> {
    pub fn new<F: Fn() -> T>(init: F) -> Self {
        let num_cpu = unsafe { crate::kernel::NUM_CPU };
        assert!(num_cpu > 0, "NUM_CPU must be set before creating PerCpu");

        let mut data = Vec::with_capacity(num_cpu);
        for _ in 0..num_cpu {
            data.push(CacheAligned::new(init()));
        }
        PerCpu {
            data,
            _marker: PhantomData,
        }
    }

    pub fn new_with_id<F: Fn(usize) -> T>(init: F) -> Self {
        let num_cpu = unsafe { crate::kernel::NUM_CPU };
        assert!(num_cpu > 0, "NUM_CPU must be set before creating PerCpu");

        let mut data = Vec::with_capacity(num_cpu);
        for i in 0..num_cpu {
            data.push(CacheAligned::new(init(i)));
        }
        PerCpu {
            data,
            _marker: PhantomData,
        }
    }

    pub fn new_with_id_and_count<F: Fn(usize) -> T>(count: usize, init: F) -> Self {
        assert!(count > 0, "CPU count must be greater than 0");

        let mut data = Vec::with_capacity(count);
        for i in 0..count {
            data.push(CacheAligned::new(init(i)));
        }
        PerCpu {
            data,
            _marker: PhantomData,
        }
    }

    /// 获取当前 CPU 的数据（只读）
    #[inline]
    pub fn get(&self) -> &T {
        let cpu_id = CPU::id();
        unsafe { &*self.data[cpu_id].get() }
    }

    /// 获取当前 CPU 的数据（可变）
    #[inline]
    #[allow(clippy::mut_from_ref)]
    pub fn get_mut(&self) -> &mut T {
        let cpu_id = CPU::id();
        unsafe { &mut *self.data[cpu_id].get() }
    }

    /// 获取指定 CPU 的数据（只读）
    #[inline]
    pub fn get_of(&self, cpu_id: usize) -> &T {
        assert!(cpu_id < self.data.len(), "Invalid CPU ID");
        unsafe { &*self.data[cpu_id].get() }
    }
}

unsafe impl<T: Send, CPU: CpuOps> Send for PerCpu<T, CPU> {}
unsafe impl<T: Send, CPU: CpuOps> Sync for PerCpu<T, CPU> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::PreemptGuard;
    use crate::{kassert, test_case};
    use core::sync::atomic::{AtomicUsize, Ordering};

    test_case!(test_per_cpu_basic, {
        let per_cpu = PerCpu::new(|| AtomicUsize::new(0));
        let _guard = PreemptGuard::new();
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
        let _guard = PreemptGuard::new();
        let value = per_cpu.get_mut();
        kassert!(*value == 0);
        *value = 100;
        kassert!(*per_cpu.get_mut() == 100);
    });
}
