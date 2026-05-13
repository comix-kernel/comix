//! LoongArch64 库模块

pub mod platform;

/// 兼容性别名：共享代码（console.rs, main.rs）通过 `crate::arch::lib::sbi::*`
/// 访问平台操作。待 HAL trait 覆盖 console/power 功能后可移除此模块。
pub mod sbi {
    pub use super::platform::*;
}
