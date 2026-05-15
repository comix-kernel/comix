//! 中断保护器
//!
//! 基于 RAII 实现中断保护。在创建时通过 `CpuOps` 禁用中断并保存之前的状态；
//! 在销毁时自动恢复之前的中断状态。
//!
//! 支持嵌套：内部使用 per-CPU 引用计数，只有最外层守卫在 drop
//! 时才真正恢复中断状态。
//!
//! # 泛型参数
//!
//! * `CPU` - 实现 `CpuOps` 的类型，默认使用 `ArchImpl`（编译时选择的架构）

use crate::arch::ArchImpl;
use crate::arch::CpuOps;
use crate::config::MAX_CPU_COUNT;
use core::marker::PhantomData;
use core::ops::Drop;
use core::sync::atomic::{AtomicUsize, Ordering};

/// 缓存行对齐的原子计数器
#[repr(align(64))]
struct CacheAlignedAtomic(AtomicUsize);

impl CacheAlignedAtomic {
    const fn new() -> Self {
        CacheAlignedAtomic(AtomicUsize::new(0))
    }
}

/// 每个 CPU 的中断保护嵌套深度
static INTR_DEPTH: [CacheAlignedAtomic; MAX_CPU_COUNT] =
    [const { CacheAlignedAtomic::new() }; MAX_CPU_COUNT];

/// 每个 CPU 保存的中断标志（仅最外层有效）
static SAVED_INTR_FLAGS: [CacheAlignedAtomic; MAX_CPU_COUNT] =
    [const { CacheAlignedAtomic::new() }; MAX_CPU_COUNT];

/// 中断保护器，基于 RAII 实现中断保护。
///
/// 在创建时原子地禁用中断并保存之前的状态；
/// 在销毁时自动恢复之前的中断状态。
///
/// 支持嵌套：内部使用 per-CPU 引用计数。
pub struct IntrGuard<CPU: CpuOps = ArchImpl> {
    _marker: PhantomData<CPU>,
    /// 此 guard 创建时是否已处于临界区内（即是否嵌套）
    was_nested: bool,
}

impl<CPU: CpuOps> IntrGuard<CPU> {
    /// 禁用中断并返回一个 IntrGuard 实例。
    ///
    /// 只有最外层的守卫会真正禁用中断。嵌套的守卫仅递增引用计数。
    /// 当最外层守卫被 drop 时，中断状态会被恢复。
    pub fn new() -> Self {
        let cpu_id = CPU::id();
        let depth = INTR_DEPTH[cpu_id].0.load(Ordering::Relaxed);
        let was_nested = depth > 0;
        if !was_nested {
            let flags = CPU::disable_interrupts();
            SAVED_INTR_FLAGS[cpu_id].0.store(flags, Ordering::Relaxed);
        }
        INTR_DEPTH[cpu_id].0.fetch_add(1, Ordering::Relaxed);
        core::sync::atomic::fence(Ordering::Acquire);
        IntrGuard {
            _marker: PhantomData,
            was_nested,
        }
    }

    /// 检查进入此临界区前，中断是否处于启用状态。
    ///
    /// 嵌套 guard 创建时中断已被外层禁用，因此返回 false。
    /// 必须在创建该守卫的同一 CPU 上调用。
    pub fn was_enabled(&self) -> bool {
        if self.was_nested {
            return false;
        }
        let cpu_id = CPU::id();
        let flags = SAVED_INTR_FLAGS[cpu_id].0.load(Ordering::Relaxed);
        CPU::interrupt_was_enabled(flags)
    }
}

impl<CPU: CpuOps> Drop for IntrGuard<CPU> {
    fn drop(&mut self) {
        let cpu_id = CPU::id();
        core::sync::atomic::fence(Ordering::Release);
        let prev = INTR_DEPTH[cpu_id].0.fetch_sub(1, Ordering::Relaxed);
        if prev == 1 {
            let flags = SAVED_INTR_FLAGS[cpu_id].0.load(Ordering::Relaxed);
            CPU::restore_interrupt_state(flags);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{arch::intr::*, kassert, println, test_case};

    test_case!(test_guard_disables_interrupts, {
        println!("Testing: test_guard_disables_interrupts");
        unsafe { enable_interrupts() };
        kassert!(are_interrupts_enabled());

        let guard = IntrGuard::<ArchImpl>::new();

        kassert!(guard.was_enabled());
        kassert!(!are_interrupts_enabled());

        drop(guard);
        kassert!(are_interrupts_enabled());
    });

    test_case!(test_guard_restores_on_drop, {
        println!("Testing: test_guard_restores_on_drop");
        let initial_flags: usize = {
            let flags = unsafe { read_and_disable_interrupts() };
            unsafe { enable_interrupts() };
            flags
        };

        kassert!(are_interrupts_enabled());

        {
            let guard = IntrGuard::<ArchImpl>::new();
            kassert!(!are_interrupts_enabled());
            kassert!(guard.was_enabled());
        }

        kassert!(are_interrupts_enabled());

        unsafe { restore_interrupts(initial_flags) };
    });

    test_case!(test_nested_intr_guard, {
        println!("Testing: test_nested_intr_guard");
        unsafe { enable_interrupts() };
        kassert!(are_interrupts_enabled());

        {
            let outer = IntrGuard::<ArchImpl>::new();
            kassert!(!are_interrupts_enabled());
            kassert!(outer.was_enabled());

            {
                let inner = IntrGuard::<ArchImpl>::new();
                kassert!(!are_interrupts_enabled());
                // 内层守卫: 进入时中断已禁用, 所以 was_enabled 应该是 false
                kassert!(!inner.was_enabled());
            }

            // 内层 drop 后中断仍应禁用 (因为外层还持有)
            kassert!(!are_interrupts_enabled());
        }

        // 外层 drop 后中断应恢复
        kassert!(are_interrupts_enabled());
    });
}
