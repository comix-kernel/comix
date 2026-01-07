//! LoongArch64 架构模块
//!
//! 包含 LoongArch64 处理器架构相关的实现。

pub mod boot;
pub mod constant;
pub mod info;
pub mod intr;
pub mod ipi;
pub mod kernel;
pub mod lib;
pub mod mm;
pub mod platform;
mod selftest;
pub mod syscall;
pub mod timer;
pub mod trap;
