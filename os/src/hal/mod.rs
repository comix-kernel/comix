//! 硬件抽象层 (HAL)
//!
//! 本模块提供架构无关的核心抽象 trait 和类型，借鉴 moss-kernel 的 HAL 设计模式。
//!
//! # 模块层次
//!
//! - [`cpu_ops`] — 最底层 CPU 操作（中断控制、核心 ID、halt）
//! - [`address`] — 类型安全的地址抽象（VA/PA/UA）
//! - [`virtual_memory`] — 虚拟内存子系统（UserAddressSpace / KernAddressSpace / VirtualMemory）
//! - [`arch`] — 顶层架构抽象（进程管理、信号、内存复制、电源管理）
//! - [`mock`] — 测试用 Mock 实现
//!
//! # 关键设计模式
//!
//! 1. **最小编译依赖** — `CpuOps` 只有 5 个方法
//! 2. **泛型注入** — 同步原语通过泛型参数注入 CPU 操作
//! 3. **关联类型** — `Arch` 和 `VirtualMemory` 用关联类型约束架构特定类型
//! 4. **Sealed trait** — 地址类型系统通过 sealed trait 建立安全边界
//! 5. **Mock + 宿主编译测试** — `#[cfg(test)]` 提供 mock impl

pub mod address;
pub mod arch;
pub mod cpu_ops;
pub mod mock;
pub mod virtual_memory;

pub use cpu_ops::CpuOps;
