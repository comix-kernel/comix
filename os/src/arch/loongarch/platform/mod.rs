//! LoongArch64 平台模块（存根）

mod loongarch_virt;

pub use loongarch_virt::*;

/// virt 平台别名（用于兼容 RISC-V 代码）
pub mod virt {
    pub use super::loongarch_virt::*;
}

/// 初始化平台
pub fn init() {
    // TODO: 初始化 LoongArch QEMU virt 平台
}
