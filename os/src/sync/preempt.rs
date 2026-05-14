//! 抢占控制
//!
//! 访问 Per-CPU 变量时需要禁用抢占，防止任务迁移导致数据不一致。
//!
//! 抢占控制函数使用 `ArchImpl` 获取 CPU ID。如需在宿主测试中使用，
//! 可使用泛型版本 `preempt_disable_generic::<CPU>()` 等。

use core::sync::atomic::{AtomicUsize, Ordering};

use crate::arch::ArchImpl;
use crate::config::MAX_CPU_COUNT;
use crate::arch::CpuOps;

/// 缓存行对齐的原子计数器
#[repr(align(64))]
struct CacheAlignedAtomic(AtomicUsize);

impl CacheAlignedAtomic {
    const fn new() -> Self {
        CacheAlignedAtomic(AtomicUsize::new(0))
    }
}

static PREEMPT_COUNT: [CacheAlignedAtomic; MAX_CPU_COUNT] =
    [const { CacheAlignedAtomic::new() }; MAX_CPU_COUNT];

// ---- 泛型版本（用于测试或显式架构选择） ----

/// 禁用抢占（泛型版本）
#[inline]
pub fn preempt_disable_generic<CPU: CpuOps>() {
    let cpu_id = CPU::id();
    PREEMPT_COUNT[cpu_id].0.fetch_add(1, Ordering::Relaxed);
    core::sync::atomic::fence(Ordering::Acquire);
}

/// 启用抢占（泛型版本）
#[inline]
pub fn preempt_enable_generic<CPU: CpuOps>() {
    core::sync::atomic::fence(Ordering::Release);
    let cpu_id = CPU::id();
    PREEMPT_COUNT[cpu_id].0.fetch_sub(1, Ordering::Relaxed);
}

/// 检查抢占是否已禁用（泛型版本）
#[inline]
pub fn preempt_disabled_generic<CPU: CpuOps>() -> bool {
    let cpu_id = CPU::id();
    PREEMPT_COUNT[cpu_id].0.load(Ordering::Relaxed) > 0
}

/// 抢占保护 RAII 守卫（泛型版本）
pub struct PreemptGuardGeneric<CPU: CpuOps> {
    _marker: core::marker::PhantomData<CPU>,
}

impl<CPU: CpuOps> PreemptGuardGeneric<CPU> {
    #[inline]
    pub fn new() -> Self {
        preempt_disable_generic::<CPU>();
        Self {
            _marker: core::marker::PhantomData,
        }
    }
}

impl<CPU: CpuOps> Drop for PreemptGuardGeneric<CPU> {
    #[inline]
    fn drop(&mut self) {
        preempt_enable_generic::<CPU>();
    }
}

// ---- 具体版本（生产使用，使用 ArchImpl） ----

/// 禁用抢占
#[inline]
pub fn preempt_disable() {
    preempt_disable_generic::<ArchImpl>();
}

/// 启用抢占
#[inline]
pub fn preempt_enable() {
    preempt_enable_generic::<ArchImpl>();
}

/// 检查抢占是否已禁用
#[inline]
pub fn preempt_disabled() -> bool {
    preempt_disabled_generic::<ArchImpl>()
}

/// 抢占保护 RAII 守卫
pub struct PreemptGuard;

impl PreemptGuard {
    #[inline]
    pub fn new() -> Self {
        preempt_disable();
        PreemptGuard
    }
}

impl Drop for PreemptGuard {
    #[inline]
    fn drop(&mut self) {
        preempt_enable();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kassert, test_case};

    test_case!(test_preempt_disable_enable, {
        kassert!(!preempt_disabled());
        preempt_disable();
        kassert!(preempt_disabled());
        preempt_disable();
        kassert!(preempt_disabled());
        preempt_enable();
        kassert!(preempt_disabled());
        preempt_enable();
        kassert!(!preempt_disabled());
    });

    test_case!(test_preempt_guard, {
        kassert!(!preempt_disabled());
        {
            let _guard = PreemptGuard::new();
            kassert!(preempt_disabled());
        }
        kassert!(!preempt_disabled());
    });

    test_case!(test_nested_preempt_guard, {
        kassert!(!preempt_disabled());
        {
            let _guard1 = PreemptGuard::new();
            kassert!(preempt_disabled());
            {
                let _guard2 = PreemptGuard::new();
                kassert!(preempt_disabled());
            }
            kassert!(preempt_disabled());
        }
        kassert!(!preempt_disabled());
    });
}
