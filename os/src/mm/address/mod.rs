// HACK: 在一个模块目录/文件的顶层又声明了一个同名子模块，这会造成 “module inception”。
// 虽然功能上可行，但会引起 API/模块层次混淆，Clippy 建议消除这种重复。
#![allow(clippy::module_inception)]
//! 地址模块
//!
//! 此模块提供了用于处理物理地址和虚拟地址，
//! 以及内存管理系统中的页码的抽象。
//!
//! # 地址类型
//!
//! - Address - 表示内存地址（物理或虚拟）的 Trait
//! - [`Paddr`] - 物理地址类型
//! - [`Vaddr`] - 虚拟地址类型
//! - [`ConvertablePaddr`] - 将物理地址转换为虚拟地址的 Trait
//! - ConvertableVaddr - 将虚拟地址转换为物理地址的 Trait
//!
//! # 地址范围
//!
//! - AddressRange - 泛型地址范围
//! - PaddrRange - 物理地址范围的类型别名
//! - VaddrRange - 虚拟地址范围的类型别名
//! - AddressRangeIterator - 地址范围的迭代器
//!
//! # 页码
//!
//! - [`PageNum`] - 表示页码的 Trait
//! - [`Ppn`] - 物理页码（Physical Page Number）
//! - [`Vpn`] - 虚拟页码（Virtual Page Number）
//!
//! # 页码范围
//!
//! - PageNumRange - 泛型页码范围
//! - [`PpnRange`] - 物理页码范围的类型别名
//! - [`VpnRange`] - 虚拟页码范围的类型别名
//! - PageNumRangeIterator - 页码范围的迭代器
//!
//! # 操作
//!
//! 此模块提供了三个关键的 Trait 类别：
//!
//! - [`UsizeConvert`] - 在类型和 usize 之间进行转换
//! - CalcOps - 算术和位操作
//! - AlignOps - 地址对齐操作
mod address;
mod operations;
mod page_num;

pub use address::{ConvertablePaddr, Paddr, Vaddr};
pub use operations::UsizeConvert;
pub use page_num::{PageNum, Ppn, PpnRange, Vpn, VpnRange};
