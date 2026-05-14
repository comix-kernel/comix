//! LoongArch64 架构模块

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

use crate::impl_arch_common;

impl_arch_common!(
    cpu_ops::LoongArch64,
    memory::LoongArch64ProcessAddressSpace,
    memory::LoongArch64KernelAddressSpace
);
