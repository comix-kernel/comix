//! 体系结构相关的模块
//!
//! 包含与特定处理器架构相关的实现。
//! 根据目标架构选择性地包含不同的子模块。

#[cfg(target_arch = "loongarch64")]
mod loongarch;

#[cfg(target_arch = "riscv64")]
mod riscv;

#[cfg(target_arch = "loongarch64")]
pub use loongarch::*;

#[cfg(target_arch = "riscv64")]
pub use riscv::*;
