//! RISC-V 内存子系统 VirtualMemory trait 实现
//!
//! 通过共享宏生成 `Riscv64KernelAddressSpace` 和 `Riscv64ProcessAddressSpace`。

use crate::impl_virtual_memory;

impl_virtual_memory!(Riscv64ProcessAddressSpace, Riscv64KernelAddressSpace);
