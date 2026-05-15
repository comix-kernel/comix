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

use crate::{impl_arch, impl_platform};

impl_arch!(
    cpu_ops::LoongArch64,
    memory::LoongArch64ProcessAddressSpace,
    memory::LoongArch64KernelAddressSpace
);

impl_platform!(cpu_ops::LoongArch64);
