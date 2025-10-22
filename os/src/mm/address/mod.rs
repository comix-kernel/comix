//! Address module
//!
//! This module provides abstractions for working with physical and virtual addresses,
//! as well as page numbers in a memory management system.
//!
//! # Address Types
//!
//! - [`Address`]: Trait for representing memory addresses (physical or virtual)
//! - [`Paddr`]: Physical address type
//! - [`Vaddr`]: Virtual address type
//! - [`ConvertablePaddr`]: Trait for converting physical addresses to virtual addresses
//! - [`ConvertableVaddr`]: Trait for converting virtual addresses to physical addresses
//!
//! # Address Ranges
//!
//! - [`AddressRange`]: Generic range of addresses
//! - [`PaddrRange`]: Type alias for physical address range
//! - [`VaddrRange`]: Type alias for virtual address range
//! - [`AddressRangeIterator`]: Iterator for address ranges
//!
//! # Page Numbers
//!
//! - [`PageNum`]: Trait for representing page numbers
//! - [`Ppn`]: Physical page number
//! - [`Vpn`]: Virtual page number
//!
//! # Page Number Ranges
//!
//! - [`PageNumRange`]: Generic range of page numbers
//! - [`PpnRange`]: Type alias for physical page number range
//! - [`VpnRange`]: Type alias for virtual page number range
//! - [`PageNumRangeIterator`]: Iterator for page number ranges
//!
//! # Operations
//!
//! The module provides three key trait categories:
//!
//! - [`UsizeConvert`]: Convert between types and usize
//! - [`CalcOps`]: Arithmetic and bitwise operations
//! - [`AlignOps`]: Address alignment operations

mod address;
mod operations;
mod page_num;

pub use address::{ConvertablePaddr, Paddr};
pub use operations::UsizeConvert;
pub use page_num::{PageNum, Ppn, PpnRange};
