// HACK: 在一个模块目录/文件的顶层又声明了一个同名子模块，这会造成 “module inception”。
// 虽然功能上可行，但会引起 API/模块层次混淆，Clippy 建议消除这种重复。
#![allow(clippy::module_inception)]
//! Address module
//!
//! This module provides abstractions for working with physical and virtual addresses,
//! as well as page numbers in a memory management system.
//!
//! # Address Types
//!
//! - `Address`: Trait for representing memory addresses (physical or virtual)
//! - [`Paddr`]: Physical address type
//! - [`Vaddr`]: Virtual address type
//! - [`ConvertablePaddr`]: Trait for converting physical addresses to virtual addresses
//! - `ConvertableVaddr`: Trait for converting virtual addresses to physical addresses
//!
//! # Address Ranges
//!
//! - `AddressRange`: Generic range of addresses
//! - `PaddrRange`: Type alias for physical address range
//! - `VaddrRange`: Type alias for virtual address range
//! - `AddressRangeIterator`: Iterator for address ranges
//!
//! # Page Numbers
//!
//! - [`PageNum`]: Trait for representing page numbers
//! - [`Ppn`]: Physical page number
//! - [`Vpn`]: Virtual page number
//!
//! # Page Number Ranges
//!
//! - `PageNumRange`: Generic range of page numbers
//! - [`PpnRange`]: Type alias for physical page number range
//! - [`VpnRange`]: Type alias for virtual page number range
//! - `PageNumRangeIterator`: Iterator for page number ranges
//!
//! # Operations
//!
//! The module provides three key trait categories:
//!
//! - [`UsizeConvert`]: Convert between types and usize
//! - `CalcOps`: Arithmetic and bitwise operations
//! - `AlignOps`: Address alignment operations

mod address;
mod operations;
mod page_num;

pub use address::{ConvertablePaddr, Paddr, Vaddr};
pub use operations::UsizeConvert;
pub use page_num::{PageNum, Ppn, PpnRange, Vpn, VpnRange};
