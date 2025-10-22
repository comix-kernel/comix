//! RISC-V architecture memory management module
//!
//! This module provides RISC-V specific implementations for memory management,
//! using the SV39 paging scheme.
//!
//! # SV39 Paging Scheme
//!
//! SV39 is a 39-bit virtual address paging scheme for RISC-V:
//! - Virtual address: 39 effective bits (bits 38-63 must match bit 38)
//! - Physical address: 56 bits (bits 0-55)
//! - Page size: 4 KiB
//!
//! # Address Translation
//!
//! This module uses direct mapping for address translation:
//! - Virtual address start: `0xffff_ffc0_0000_0000`
//! - Physical addresses are extracted using bitwise AND with `PADDR_MASK`
//! - Virtual addresses are created using bitwise OR with `VADDR_START`

mod page_table;
mod page_table_entry;

pub use page_table::PageTableInner;
pub use page_table_entry::PageTableEntry;

/// starting address of the virtual address space in SV39
///
/// This constant defines the starting position of the kernel's high virtual address space.
/// In the SV39 paging scheme, this is a valid higher-half kernel address.
pub const VADDR_START: usize = 0xffff_ffc0_0000_0000;

/// physical address mask for extracting physical address from virtual address
///
/// This mask preserves the lower 38 bits (bits 0-37), which corresponds to
/// the physical address space size in SV39.
pub const PADDR_MASK: usize = 0x0000_3fff_ffff_ffff;

/// convert virtual address to physical address
///
/// # Parameters
///
/// * `vaddr` - virtual address
///
/// # Returns
///
/// The corresponding physical address
///
/// # Note
///
/// This function must be implemented in all architecture-specific mm modules.
pub const fn vaddr_to_paddr(vaddr: usize) -> usize {
    vaddr & PADDR_MASK
}

/// convert physical address to virtual address
///
/// # Parameters
///
/// * `paddr` - physical address
///
/// # Returns
///
/// The corresponding virtual address
///
/// # Note
///
/// This function must be implemented in all architecture-specific mm modules.
pub const fn paddr_to_vaddr(paddr: usize) -> usize {
    paddr | VADDR_START
}
