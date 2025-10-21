use crate::arch::{constant::SSTATUS_SIE, intr::{read_and_disable_interrupts, restore_interrupts}};
use core::ops::Drop;

/// 中断保护器，基于 RAII 实现中断保护。
/// 在创建时原子地禁用中断并保存之前的状态；
/// 在销毁时自动恢复之前的中断状态。
/// 不可重入 (即不能嵌套调用 IntrGuard::new())。
pub struct IntrGuard {
    flags: usize,
}

impl IntrGuard {
    /// 原子地禁用中断并返回一个 IntrGuard 实例。
    /// 该实例在离开作用域时会自动恢复中断状态。
    pub fn new() -> Self {
        let flags = unsafe { 
            read_and_disable_interrupts() 
        };
        IntrGuard { flags }
    }

    /// 检查进入临界区前，中断是否处于启用状态。
    pub fn was_enabled(&self) -> bool {
        self.flags & SSTATUS_SIE != 0
    }
}

/// 当 IntrGuard 离开作用域时，自动恢复中断状态。
impl Drop for IntrGuard {
    fn drop(&mut self) {
        unsafe { restore_interrupts(self.flags) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::arch::intr::*; 

    /// 测试 IntrGuard::new() 是否成功禁用中断，并检查 was_enabled
    #[test_case]
    fn test_guard_disables_interrupts() {
        println!("Testing: test_guard_disables_interrupts");
        unsafe { enable_interrupts() }; 
        assert!(are_interrupts_enabled(), "初始环境：中断应为启用状态");

        let guard = IntrGuard::new();
        
        assert!(guard.was_enabled(), "was_enabled应为true (进入前启用)");
        
        assert!(!are_interrupts_enabled(), "临界区内：中断应为禁用状态");
    }

    /// 测试 IntrGuard 在离开作用域时是否恢复中断状态
    #[test_case]
    fn test_guard_restores_on_drop() {
        println!("Testing: test_guard_restores_on_drop");
        let initial_flags: usize = {
            let flags = unsafe { read_and_disable_interrupts() }; 
            unsafe { enable_interrupts() };
            flags
        };
        
        let initial_state = are_interrupts_enabled();
        
        assert!(initial_state);

        {
            let guard = IntrGuard::new();
            assert!(!are_interrupts_enabled());
            
            assert!(guard.flags & SSTATUS_SIE != 0);
        }

        assert!(are_interrupts_enabled());

        unsafe { restore_interrupts(initial_flags) };
    }
}