//! SUM (permit Supervisor User Memory access) 位保护器
//!
//! 提供 RAII 方式管理 sstatus 寄存器的 SUM 位，确保在访问用户空间内存时
//! 正确设置和清除 SUM 位，即使发生 panic 也能正确恢复。

use core::ops::Drop;
use riscv::register::sstatus;

/// SUM 位保护器，基于 RAII 实现用户空间内存访问保护。
///
/// 在创建时保存 sstatus.SUM 位的当前状态并设置为 1（允许内核访问用户空间内存）；
/// 在销毁时自动恢复之前保存的状态。
///
/// 这种设计允许安全的嵌套使用：
/// - 如果外层已经设置了 SUM 位，内层 guard 不会重复设置，也不会在销毁时清除
/// - 只有最外层的 guard 会在销毁时清除 SUM 位
///
/// # Safety
///
/// 此 guard 必须在访问用户空间内存之前创建，并在访问完成后立即销毁。
/// 不应该长时间持有此 guard，因为它会降低内核的安全性。
///
/// # 使用示例
///
/// ```ignore
/// // 读取用户空间指针
/// let user_value = {
///     let _guard = SumGuard::new();
///     unsafe { core::ptr::read(user_ptr) }
/// }; // 离开作用域，自动恢复 SUM 位
/// ```
///
/// # 为什么需要此 guard
///
/// 手动调用 `sstatus::set_sum()` 和 `sstatus::clear_sum()` 存在安全隐患：
/// 如果在两次调用之间发生 panic（例如，由于无效的用户指针导致缺页异常无法处理），
/// `clear_sum()` 将不会被执行，导致 SUM 位保持置位状态。这会使内核在后续执行中
/// 意外地允许访问用户空间内存，可能导致安全漏洞。
///
/// 使用此 RAII guard，即使发生 panic，Rust 的 drop 机制也会确保 SUM 位被正确恢复。
pub struct SumGuard {
    /// 创建 guard 前 SUM 位是否已设置
    was_set: bool,
}

impl SumGuard {
    /// 创建一个新的 SumGuard，保存当前 SUM 位状态并设置为 1。
    ///
    /// # Safety
    ///
    /// 调用者必须确保：
    /// 1. 即将访问的用户空间地址是有效的
    /// 2. 不会长时间持有此 guard
    /// 3. 在 guard 生命周期内访问的所有用户空间指针都已经过验证
    #[inline]
    pub fn new() -> Self {
        // 保存当前 SUM 位状态
        let was_set = sstatus::read().sum();

        // 如果 SUM 位尚未设置，则设置它
        if !was_set {
            // SAFETY: 设置 SUM 位以允许访问用户空间内存
            unsafe { sstatus::set_sum() };
        }

        SumGuard { was_set }
    }

    /// 检查在创建此 guard 前，SUM 位是否已经被设置
    #[allow(dead_code)]
    pub fn was_set(&self) -> bool {
        self.was_set
    }
}

impl Drop for SumGuard {
    /// 当 SumGuard 离开作用域时，恢复之前保存的 SUM 位状态。
    #[inline]
    fn drop(&mut self) {
        // 只有在创建 guard 前 SUM 位未设置时，才清除它
        // 这确保了嵌套使用的正确性
        if !self.was_set {
            // SAFETY: 恢复之前的状态
            unsafe { sstatus::clear_sum() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kassert, println, test_case};

    // 测试 SumGuard::new() 是否成功设置 SUM 位
    test_case!(test_guard_sets_sum, {
        println!("Testing: test_guard_sets_sum");

        // 确保初始状态下 SUM 未设置
        unsafe { sstatus::clear_sum() };
        kassert!(!sstatus::read().sum());

        {
            let _guard = SumGuard::new();
            // SUM 位应该已设置
            kassert!(sstatus::read().sum());
        }

        // 离开作用域后 SUM 位应该被清除
        kassert!(!sstatus::read().sum());
    });

    // 测试嵌套 SumGuard（现在应该能正确工作）
    test_case!(test_guard_nested, {
        println!("Testing: test_guard_nested");

        unsafe { sstatus::clear_sum() };
        kassert!(!sstatus::read().sum());

        {
            let guard1 = SumGuard::new();
            kassert!(sstatus::read().sum());
            kassert!(!guard1.was_set()); // 第一个 guard 设置了 SUM 位

            {
                let guard2 = SumGuard::new();
                kassert!(sstatus::read().sum());
                kassert!(guard2.was_set()); // 第二个 guard 发现 SUM 已设置
            }

            // 内层 guard 销毁后，SUM 位应该仍然为 1（因为外层还需要它）
            kassert!(sstatus::read().sum());
        }

        // 外层 guard 销毁后，SUM 位才被清除
        kassert!(!sstatus::read().sum());
    });

    // 测试 panic 时 SumGuard 是否能正确清理
    // 注意：此测试需要 panic handler 支持
    // test_case!(test_guard_cleans_on_panic, {
    //     println!("Testing: test_guard_cleans_on_panic");
    //     // 此测试需要特殊的 panic 处理机制
    // });
}
