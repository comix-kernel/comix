//! Address module
//!
//! This module provides abstractions for working with physical and virtual addresses,
//! as well as page numbers in a memory management system.
//!
//! # Components
//!
//! - [`Address`]: Trait for representing memory addresses (physical or virtual)
//! - [`Paddr`]: Physical address type
//! - [`Vaddr`]: Virtual address type
//! - [`AddressRange`]: Generic range of addresses
//! - [`PageNum`]: Trait for representing page numbers
//! - [`Ppn`]: Physical page number
//! - [`Vpn`]: Virtual page number
//! - [`PageNumRange`]: Generic range of page numbers
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

pub use address::{
    Address, AddressRange, ConvertablePaddr, ConvertableVaddr, Paddr, PaddrRange, Vaddr, VaddrRange,
};
pub use operations::{AlignOps, CalcOps, UsizeConvert};
pub use page_num::{PageNum, Ppn, Vpn};
