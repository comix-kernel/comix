// TODO: Complete the comments below
//! MemoryManagement module for RISC-V architecture
//! 
//! This module provides RISC-V specific implementations for memory management,
//! using SV39 paging scheme. 

pub const VADDR_START: usize = 0xffff_ffc0_0000_0000;
pub const PADDR_MASK: usize = 0x0000_3fff_ffff_ffff;

// MUST implemented in any arch-specific mm module
/// Convert virtual address to physical address
pub const fn vaddr_to_paddr(vaddr: usize) -> usize {
    vaddr & PADDR_MASK
}

// MUST implemented in any arch-specific mm module
/// Convert physical address to virtual address
pub const fn paddr_to_vaddr(paddr: usize) -> usize {
    paddr | VADDR_START
}