//! RISC-V 架构相关模块
pub mod boot;
pub mod constant;
pub mod cpu_ops;
pub mod intr;
pub mod ipi;
pub mod kernel;
pub mod lib;
pub mod memory;
pub mod mm;
pub mod platform;
pub mod timer;
pub mod trap;

use crate::{impl_arch, impl_platform};

impl_arch!(
    cpu_ops::Riscv64,
    memory::Riscv64ProcessAddressSpace,
    memory::Riscv64KernelAddressSpace
);

impl_platform!(cpu_ops::Riscv64);
