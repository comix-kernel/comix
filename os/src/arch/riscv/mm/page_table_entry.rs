use crate::mm::address::{Ppn, UsizeConvert};
use crate::mm::page_table::PageTableEntry as PageTableEntryTrait;
use crate::mm::page_table::{UniversalConvertableFlag, UniversalPTEFlag};

bitflags::bitflags! {
    pub struct SV39PTEFlags: usize {
        const VALID = 1 << 0;         // Indicates whether the entry is valid
        const READ = 1 << 1;          // Indicates whether the page is readable
        const WRITE = 1 << 2;         // Indicates whether the page is writeable
        const EXECUTE = 1 << 3;       // Indicates whether the page is executable
        const USER = 1 << 4;          // Indicates whether the page is accessible from user mode
        const GLOBAL = 1 << 5;        // Indicates whether the page is global
        const ACCESSED = 1 << 6;      // Indicates whether the page has been accessed
        const DIRTY = 1 << 7;         // Indicates whether the page has been written to
        const _RESERVED = 1 << 8;     // Reserved for future use (according to SV39 spec)
    }
}



/*
 * SV39 Page Table Entry (PTE) format:
 * ------------------------------------------------
 * | Bits  | Description                           |
 * ------------------------------------------------
 * | 0-7   | Flags (Valid, Read, Write, Execute,  |
 * |       | User, Global, Accessed, Dirty)       |
 * ------------------------------------------------
 * | 8-9   | Reserved (must be zero)              |
 * ------------------------------------------------
 * | 10-53 | Physical Page Number (PPN)           |
 * ------------------------------------------------
 * | 54-63 | Reserved (must be zero)              |
 * ------------------------------------------------
 */

const SV39_PTE_FLAG_MASK: usize = 0xff; // Lower 8 bits for SV39 PTE flags
const SV39_PTE_PPN_OFFSET: usize = 10;  // PPN starts from bit 10
const SV39_PTE_PPN_MASK: u64 = 0x000f_ffff_ffff_c00; // Bits 10-53 for PPN

impl UniversalConvertableFlag for SV39PTEFlags {
    fn from_universal(flag: UniversalPTEFlag) -> Self {
        Self::from_bits(flag.bits() & SV39_PTE_FLAG_MASK).unwrap()
    }

    fn to_universal(&self) -> UniversalPTEFlag {
        UniversalPTEFlag::from_bits(self.bits() & SV39_PTE_FLAG_MASK).unwrap()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PageTableEntry(u64);

impl PageTableEntryTrait for PageTableEntry {
    type Bits = u64;

    fn from_bits(bits: Self::Bits) -> Self {
        PageTableEntry(bits)
    }

    fn to_bits(&self) -> Self::Bits {
        self.0
    }

    fn empty() -> Self {
        PageTableEntry(0)
    }

    fn new_leaf(ppn: Ppn, flags: UniversalPTEFlag) -> Self {
        let ppn_bits: u64 = ppn.as_usize() as u64;
        let sv39_flags = SV39PTEFlags::from_universal(flags);
        PageTableEntry((ppn_bits << SV39_PTE_PPN_OFFSET) | (sv39_flags.bits() as u64))
    }

    fn new_table(ppn: Ppn) -> Self {
        let ppn_bits: u64 = ppn.as_usize() as u64;
        let sv39_flags = SV39PTEFlags::VALID; // Table entries must be valid
        PageTableEntry((ppn_bits << SV39_PTE_PPN_OFFSET) | (sv39_flags.bits() as u64))
    }

    fn is_valid(&self) -> bool {
        (self.0 & SV39PTEFlags::VALID.bits() as u64) != 0
    }

    fn is_huge(&self) -> bool {
        // In SV39, we can't directly determine huge pages from the PTE alone.
        let sv39_flags = SV39PTEFlags::from_bits((self.0 & SV39_PTE_FLAG_MASK as u64) as usize).unwrap();
        sv39_flags.intersects(SV39PTEFlags::union(SV39PTEFlags::READ, SV39PTEFlags::EXECUTE).union(SV39PTEFlags::WRITE))
    }

    fn is_empty(&self) -> bool {
        self.0 == 0
    }

    fn ppn(&self) -> Ppn {
        let ppn = (self.0 & SV39_PTE_PPN_MASK) >> SV39_PTE_PPN_OFFSET;
        Ppn::from_usize(ppn as usize)
    }

    fn flags(&self) -> UniversalPTEFlag {
        let sv39_flags = SV39PTEFlags::from_bits((self.0 & SV39_PTE_FLAG_MASK as u64) as usize).unwrap();
        sv39_flags.to_universal()
    }

    fn set_ppn(&mut self, ppn: Ppn) {
        let ppn_bits = ppn.as_usize() as u64;
        self.0 = (self.0 & !SV39_PTE_PPN_MASK) | (ppn_bits << SV39_PTE_PPN_OFFSET);
    }

    fn set_flags(&mut self, flags: UniversalPTEFlag) {
        let sv39_flags = SV39PTEFlags::from_universal(flags);
        self.0 = (self.0 & !(SV39_PTE_FLAG_MASK as u64)) | (sv39_flags.bits() as u64);
    }

    fn clear(&mut self) {
        self.0 = 0;
    }

    // current_flags & !flags
    fn remove_flags(&mut self, flags: UniversalPTEFlag) {
        let current_flags = self.flags();
        let new_flags = current_flags & !flags;
        self.set_flags(new_flags);
    }

    // current_flags | flags
    fn add_flags(&mut self, flags: UniversalPTEFlag) {
        self.set_flags(self.flags() | flags);
    }
}
