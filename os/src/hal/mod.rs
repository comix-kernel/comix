//! 硬件抽象层 (HAL)
//!
//! 本模块提供架构无关的纯类型抽象。
//! trait 定义（CpuOps、VirtualMemory、Arch）已移至 [`crate::arch`]。
//!
//! # 内容
//!
//! - [`address`] — 类型安全的地址抽象（VA/PA/UA）

pub mod address;
