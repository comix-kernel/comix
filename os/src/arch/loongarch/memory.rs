//! LoongArch 内存子系统 VirtualMemory trait 实现
//!
//! 通过共享宏生成 `LoongArch64KernelAddressSpace` 和 `LoongArch64ProcessAddressSpace`。

use crate::impl_virtual_memory;

impl_virtual_memory!(LoongArch64ProcessAddressSpace, LoongArch64KernelAddressSpace);
