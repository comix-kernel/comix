#![allow(dead_code)]
use crate::mm::address::Ppn;

bitflags::bitflags! {
    /// Designs a universal set of page table entry flags that can be mapped to various architectures.
    /// Be same to Risc-V SV39 in lower 8 bits.(add more flag-bit if needed for other archs)
    pub struct UniversalPTEFlag: usize {

        // ---- RISC-V SV39 compatible flags ----
        const VALID = 1 << 0;               // Indicates whether the entry is valid
        const READABLE = 1 << 1;            // Indicates whether the page is readable
        const WRITEABLE = 1 << 2;           // Indicates whether the page is writeable
        const EXECUTABLE = 1 << 3;          // Indicates whether the page is executable
        const USER_ACCESSIBLE = 1 << 4;      // Indicates whether the page is accessible from user mode
        const GLOBAL = 1 << 5;              // Indicates whether the page is global
        const ACCESSED = 1 << 6;            // Indicates whether the page has been accessed
        const DIRTY = 1 << 7;               // Indicates whether the page has been written to

        // ---- Additional universal flags ----
        #[allow(dead_code)] // TODO(暂时注释): 大页支持已暂时禁用
        const HUGE = 1 << 8;                // Indicates whether the page is a huge page ()
    }
}

impl UniversalPTEFlag {
    /// constructs a flag set for user read-only access
    pub const fn user_read() -> Self {
        Self::VALID
            .union(Self::READABLE)
            .union(Self::USER_ACCESSIBLE)
    }

    /// constructs a flag set for user read-write access
    pub const fn user_rw() -> Self {
        Self::VALID
            .union(Self::READABLE)
            .union(Self::WRITEABLE)
            .union(Self::USER_ACCESSIBLE)
    }

    /// constructs a flag set for user read-execute access
    pub const fn user_rx() -> Self {
        Self::VALID
            .union(Self::READABLE)
            .union(Self::EXECUTABLE)
            .union(Self::USER_ACCESSIBLE)
    }

    /// constructs a flag set for kernel read-write access
    pub const fn kernel_rw() -> Self {
        Self::VALID.union(Self::READABLE).union(Self::WRITEABLE)
    }

    /// constructs a flag set for kernel read-only access
    pub const fn kernel_r() -> Self {
        Self::VALID.union(Self::READABLE)
    }

    /// constructs a flag set for kernel read-execute access
    pub const fn kernel_rx() -> Self {
        Self::VALID.union(Self::READABLE).union(Self::EXECUTABLE)
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
    fn new_leaf(ppn: Ppn, flags: UniversalPTEFlag) -> Self;
    fn new_table(ppn: Ppn) -> Self;

    fn is_valid(&self) -> bool;
    fn is_huge(&self) -> bool;
    fn is_empty(&self) -> bool;

    fn ppn(&self) -> Ppn;
    fn flags(&self) -> UniversalPTEFlag;

    fn set_ppn(&mut self, ppn: Ppn);
    fn set_flags(&mut self, flags: UniversalPTEFlag);
    fn clear(&mut self);

    // current_flags & !flags
    fn remove_flags(&mut self, flags: UniversalPTEFlag);

    // current_flags | flags
    fn add_flags(&mut self, flags: UniversalPTEFlag);
}
