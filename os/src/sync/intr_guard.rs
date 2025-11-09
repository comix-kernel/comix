use crate::arch::{
    constant::SSTATUS_SIE,
    intr::{read_and_disable_interrupts, restore_interrupts},
};
use core::ops::Drop;

/// 中断保护器，基于 RAII 实现中断保护。
/// 在创建时原子地禁用中断并保存之前的状态；
/// 在销毁时自动恢复之前的中断状态。
/// 不可重入 (即不能嵌套调用 IntrGuard::new())。
/// 使用示例：
/// ```ignore
/// {
///     let guard = IntrGuard::new(); // 禁用中断
///     // 临界区代码
/// } // 离开作用域，自动恢复中断状态
/// ```
pub struct IntrGuard {
    flags: usize,
}

impl IntrGuard {
    /// 原子地禁用中断并返回一个 IntrGuard 实例。
    /// 该实例在离开作用域时会自动恢复中断状态。
    pub fn new() -> Self {
        // SAFETY: 调用者必须确保在创建 IntrGuard 实例时，
        // 没有其他代码会修改中断状态，从而保证不可重入性。
        let flags = unsafe { read_and_disable_interrupts() };
        IntrGuard { flags }
    }

    /// 检查进入临界区前，中断是否处于启用状态。
    /// 返回值：中断是否处于启用状态
    #[allow(dead_code)]
    pub fn was_enabled(&self) -> bool {
        self.flags & SSTATUS_SIE != 0
    }
}

impl Drop for IntrGuard {
    /// 当 IntrGuard 离开作用域时，自动恢复中断状态。
    fn drop(&mut self) {
        // SAFETY: flags 是在创建 IntrGuard 时保存的，
        // 因此恢复操作是安全的。
        unsafe { restore_interrupts(self.flags) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{arch::intr::*, kassert, println, test_case};

    // 测试 IntrGuard::new() 是否成功禁用中断，并检查 was_enabled
    test_case!(test_guard_disables_interrupts, {
        println!("Testing: test_guard_disables_interrupts");
        unsafe { enable_interrupts() };
        kassert!(are_interrupts_enabled());

        let guard = IntrGuard::new();

        kassert!(guard.was_enabled());

        kassert!(!are_interrupts_enabled());
    });

    // 测试 IntrGuard 在离开作用域时是否恢复中断状态
    test_case!(test_guard_restores_on_drop, {
        println!("Testing: test_guard_restores_on_drop");
        let initial_flags: usize = {
            let flags = unsafe { read_and_disable_interrupts() };
            unsafe { enable_interrupts() };
            flags
        };

        let initial_state = are_interrupts_enabled();

        kassert!(initial_state);

        {
            let guard = IntrGuard::new();
            kassert!(!are_interrupts_enabled());

            kassert!(guard.flags & SSTATUS_SIE != 0);
        }

        kassert!(are_interrupts_enabled());

        unsafe { restore_interrupts(initial_flags) };
    });
}
