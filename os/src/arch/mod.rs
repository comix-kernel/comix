//! 体系结构相关的模块
//!
//! 包含与特定处理器架构相关的实现。
//! 根据目标架构选择性地包含不同的子模块。
//!
//! # 分层约定
//!
//! 为了减少在 `arch/` 之外散落的 `cfg(target_arch = ...)` 与架构特定依赖：
//! - **架构条件编译应尽量集中在本模块**（选择 `riscv/` 或 `loongarch/`）。
//! - `arch/` 外部代码应通过 `crate::arch::*` 暴露的统一接口/钩子访问架构差异，
//!   避免直接依赖 `riscv`、`loongArch64` 等架构专用 crate 或寄存器操作。

#[cfg(target_arch = "loongarch64")]
mod loongarch;

#[cfg(target_arch = "riscv64")]
mod riscv;

#[cfg(target_arch = "loongarch64")]
pub use loongarch::*;

#[cfg(target_arch = "riscv64")]
pub use riscv::*;
