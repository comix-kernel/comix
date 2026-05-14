//! 架构 mock stub 模块
//!
//! 当目标不是 RISC-V 或 LoongArch 时（例如 x86_64 宿主测试），
//! 提供 mock 实现使得架构无关代码可以编译和测试。

pub mod arch;
pub mod boot;
pub mod constant;
pub mod intr;
pub mod ipi;
pub mod kernel;
pub mod lib;
pub mod mm;
pub mod platform;
pub mod timer;
pub mod trap;

pub use arch::MockAddressSpace;
pub use arch::MockArch;
pub use arch::MockCpuOps;
