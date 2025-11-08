//! RISC-V 架构内存管理模块
//!
//! 此模块提供了针对 **RISC-V 架构** 的内存管理实现，
//! 使用 **SV39 分页方案 (SV39 Paging Scheme)**。
//!
//! # SV39 分页方案
//!
//! SV39 是 RISC-V 上的一个 39 位虚拟地址分页方案：
//! - **虚拟地址 (Virtual address)**: 39 个有效位 (位 38-63 必须与位 38 匹配)
//! - **物理地址 (Physical address)**: 56 位 (位 0-55)
//! - **页大小 (Page size)**: 4 KiB
//!
//! # 地址转换
//!
//! 此模块使用**直接映射 (direct mapping)** 进行地址转换：
//! - 虚拟地址起始点 (Virtual address start): `0xffff_ffc0_0000_0000`
//! - **提取物理地址** 使用与 `PADDR_MASK` 进行**位与 (bitwise AND)** 操作
//! - **创建虚拟地址** 使用与 `VADDR_START` 进行**位或 (bitwise OR)** 操作

mod page_table; // 模块：页表
mod page_table_entry; // 模块：页表项

pub use page_table::PageTableInner; // 导出：页表内部结构
pub use page_table_entry::PageTableEntry; // 导出：页表项结构体

/// SV39 中虚拟地址空间的起始地址
///
/// 此常量定义了内核高位虚拟地址空间的起始位置。
/// 在 SV39 分页方案中，这是一个合法的**高半区 (higher-half)** 内核地址。
pub const VADDR_START: usize = 0xffff_ffc0_0000_0000;

/// 物理地址掩码，用于从虚拟地址中提取物理地址
///
/// 此掩码保留了低 38 位 (位 0-37)，这对应于
/// SV39 中的物理地址空间大小。
pub const PADDR_MASK: usize = 0x0000_003f_ffff_ffff;

/// 转换虚拟地址到物理地址
///
/// # 参数
///
/// * `vaddr` - 虚拟地址
///
/// # 返回
///
/// 对应的物理地址
///
/// # 注意
///
/// 此函数必须在所有架构特定的内存管理模块中实现。
pub const unsafe fn vaddr_to_paddr(vaddr: usize) -> usize {
    vaddr & PADDR_MASK
}

/// 转换物理地址到虚拟地址
///
/// # 参数
///
/// * `paddr` - 物理地址
///
/// # 返回
///
/// 对应的虚拟地址
///
/// # 注意
///
/// 此函数必须在所有架构特定的内存管理模块中实现。
pub const fn paddr_to_vaddr(paddr: usize) -> usize {
    paddr | VADDR_START
}
