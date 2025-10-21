use crate::mm::address::Paddr;

bitflags::bitflags! {
    /// Designs a universal set of page table entry flags that can be mapped to various architectures.
    /// Be same to Risc-V SV39 for templarity.(add more flag-bit if needed for other archs)
    pub struct UniversalPTEFlag: usize {
        const Valid = 1 << 0;               // Indicates whether the entry is valid
        const Readable = 1 << 1;            // Indicates whether the page is readable
        const Writeable = 1 << 2;           // Indicates whether the page is writeable
        const Executable = 1 << 3;          // Indicates whether the page is executable
        const UserAccessible = 1 << 4;      // Indicates whether the page is accessible from user mode
        const Global = 1 << 5;              // Indicates whether the page is global
        const Accessed = 1 << 6;            // Indicates whether the page has been accessed
        const Dirty = 1 << 7;               // Indicates whether the page has been written to
    }
}

pub trait UniversalConvertableFlag {
    fn from_universal(flag: UniversalPTEFlag) -> Self;
    fn to_universal(&self) -> UniversalPTEFlag;
}

pub trait PageTableEntry {

}
