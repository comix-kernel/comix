use crate::mm::address::Paddr;

bitflags::bitflags! {
    /// Designs a universal set of page table entry flags that can be mapped to various architectures.
    /// Be same to Risc-V SV39 in lower 8 bits.(add more flag-bit if needed for other archs)
    pub struct UniversalPTEFlag: usize {

        // ---- RISC-V SV39 compatible flags ----
        const Valid = 1 << 0;               // Indicates whether the entry is valid
        const Readable = 1 << 1;            // Indicates whether the page is readable
        const Writeable = 1 << 2;           // Indicates whether the page is writeable
        const Executable = 1 << 3;          // Indicates whether the page is executable
        const UserAccessible = 1 << 4;      // Indicates whether the page is accessible from user mode
        const Global = 1 << 5;              // Indicates whether the page is global
        const Accessed = 1 << 6;            // Indicates whether the page has been accessed
        const Dirty = 1 << 7;               // Indicates whether the page has been written to

        // ---- Additional universal flags ----
        const Huge = 1 << 8;                // Indicates whether the page is a huge page ()
    }
}

pub trait UniversalConvertableFlag {
    fn from_universal(flag: UniversalPTEFlag) -> Self;
    fn to_universal(&self) -> UniversalPTEFlag;
}

pub trait PageTableEntry {
    type Bits;
                                                                                                                                                                                                                                                                                                                                                                                 
    fn from_bits(bits: Self::Bits) -> Self;
    fn to_bits(&self) -> Self::Bits;
    fn empty() -> Self;
    fn new_leaf(paddr: Paddr, flags: UniversalPTEFlag) -> Self;
    fn new_table(paddr: Paddr) -> Self;

    fn is_valid(&self) -> bool;
    fn is_huge(&self) -> bool;
    fn is_empty(&self) -> bool;

    fn paddr(&self) -> Paddr;
    fn flags(&self) -> UniversalPTEFlag;

    fn set_paddr(&mut self, paddr: Paddr);
    fn set_flags(&mut self, flags: UniversalPTEFlag);
    fn clear(&mut self);

    // current_flags & !flags
    fn remove_flags(&mut self, flags: UniversalPTEFlag);

    // current_flags | flags
    fn add_flags(&mut self, flags: UniversalPTEFlag);
}
