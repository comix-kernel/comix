//! 抢占控制
//!
//! 访问 Per-CPU 变量时需要禁用抢占，防止任务迁移导致数据不一致。

use core::sync::atomic::{AtomicUsize, Ordering};

use crate::config::MAX_CPU_COUNT;

/// 缓存行对齐的原子计数器
///
/// 确保每个 CPU 的抢占计数器独占一个缓存行，避免伪共享。
#[repr(align(64))]
struct CacheAlignedAtomic(AtomicUsize);

impl CacheAlignedAtomic {
    const fn new() -> Self {
        CacheAlignedAtomic(AtomicUsize::new(0))
    }
}

/// Per-CPU 抢占计数器
///
/// 每个 CPU 维护一个计数器，> 0 表示抢占已禁用。
static PREEMPT_COUNT: [CacheAlignedAtomic; MAX_CPU_COUNT] = {
    const INIT: CacheAlignedAtomic = CacheAlignedAtomic::new();
    [INIT; MAX_CPU_COUNT]
};

/// 禁用抢占
///
/// 可以嵌套调用，每次调用增加计数器。
#[inline]
pub fn preempt_disable() {
    let cpu_id = crate::arch::kernel::cpu::cpu_id();
    PREEMPT_COUNT[cpu_id].0.fetch_add(1, Ordering::Relaxed);
    // Acquire 屏障，确保后续访问不会被重排到此之前
    core::sync::atomic::fence(Ordering::Acquire);
}

/// 启用抢占
///
/// 必须与 preempt_disable() 配对使用。
#[inline]
pub fn preempt_enable() {
    // Release 屏障，确保之前的访问不会被重排到此之后
    core::sync::atomic::fence(Ordering::Release);
    let cpu_id = crate::arch::kernel::cpu::cpu_id();
    PREEMPT_COUNT[cpu_id].0.fetch_sub(1, Ordering::Relaxed);
}

/// 检查抢占是否已禁用
#[inline]
pub fn preempt_disabled() -> bool {
    let cpu_id = crate::arch::kernel::cpu::cpu_id();
    PREEMPT_COUNT[cpu_id].0.load(Ordering::Relaxed) > 0
}

/// 抢占保护 RAII 守卫
///
/// 创建时禁用抢占，销毁时自动启用抢占。
pub struct PreemptGuard;

impl PreemptGuard {
    /// 创建守卫并禁用抢占
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
