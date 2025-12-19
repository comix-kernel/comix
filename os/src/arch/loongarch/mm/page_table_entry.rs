//! LoongArch64 页表项定义（存根）

use crate::mm::address::{Ppn, UsizeConvert};
use crate::mm::page_table::PageTableEntry as PageTableEntryTrait;
use crate::mm::page_table::{UniversalConvertableFlag, UniversalPTEFlag};

bitflags::bitflags! {
    /// LoongArch 页表项标志位
    pub struct LAPTEFlags: usize {
        /// 有效位
        const VALID = 1 << 0;
        /// 脏位
        const DIRTY = 1 << 1;
        /// 特权级 0
        const PLV0 = 0 << 2;
        /// 特权级 3 (用户态)
        const PLV3 = 3 << 2;
        /// 内存访问类型
        const MAT = 1 << 4;
        /// 可写
        const WRITE = 1 << 8;
        /// 可执行
        const EXECUTE = 1 << 11;
        /// 全局位
        const GLOBAL = 1 << 6;
    }
}

impl UniversalConvertableFlag for LAPTEFlags {
    fn from_universal(flag: UniversalPTEFlag) -> Self {
        let mut result = LAPTEFlags::empty();
        if flag.contains(UniversalPTEFlag::VALID) {
            result |= LAPTEFlags::VALID;
        }
        if flag.contains(UniversalPTEFlag::WRITEABLE) {
            result |= LAPTEFlags::WRITE;
        }
        if flag.contains(UniversalPTEFlag::EXECUTABLE) {
            result |= LAPTEFlags::EXECUTE;
        }
        if flag.contains(UniversalPTEFlag::USER_ACCESSIBLE) {
            result |= LAPTEFlags::PLV3;
        }
        if flag.contains(UniversalPTEFlag::DIRTY) {
            result |= LAPTEFlags::DIRTY;
        }
        if flag.contains(UniversalPTEFlag::GLOBAL) {
            result |= LAPTEFlags::GLOBAL;
        }
        result
    }

    fn to_universal(&self) -> UniversalPTEFlag {
        let mut result = UniversalPTEFlag::empty();
        if self.contains(LAPTEFlags::VALID) {
            result |= UniversalPTEFlag::VALID;
        }
        if self.contains(LAPTEFlags::WRITE) {
            result |= UniversalPTEFlag::WRITEABLE;
        }
        if self.contains(LAPTEFlags::EXECUTE) {
            result |= UniversalPTEFlag::EXECUTABLE;
        }
        if self.contains(LAPTEFlags::PLV3) {
            result |= UniversalPTEFlag::USER_ACCESSIBLE;
        }
        if self.contains(LAPTEFlags::DIRTY) {
            result |= UniversalPTEFlag::DIRTY;
        }
        if self.contains(LAPTEFlags::GLOBAL) {
            result |= UniversalPTEFlag::GLOBAL;
        }
        result
    }
}

/// LoongArch 页表项
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
        let la_flags = LAPTEFlags::from_universal(flags);
        PageTableEntry((ppn_bits << 12) | (la_flags.bits() as u64))
    }

    fn new_table(ppn: Ppn) -> Self {
        let ppn_bits: u64 = ppn.as_usize() as u64;
        PageTableEntry((ppn_bits << 12) | LAPTEFlags::VALID.bits() as u64)
    }

    fn is_valid(&self) -> bool {
        (self.0 & LAPTEFlags::VALID.bits() as u64) != 0
    }

    fn is_huge(&self) -> bool {
        // TODO: 实现巨页检测
        false
    }

    fn is_empty(&self) -> bool {
        self.0 == 0
    }

    fn ppn(&self) -> Ppn {
        Ppn::from_usize((self.0 >> 12) as usize)
    }

    fn flags(&self) -> UniversalPTEFlag {
        let la_flags = LAPTEFlags::from_bits(self.0 as usize & 0xfff).unwrap_or(LAPTEFlags::empty());
        la_flags.to_universal()
    }

    fn set_ppn(&mut self, ppn: Ppn) {
        let ppn_bits = ppn.as_usize() as u64;
        self.0 = (self.0 & 0xfff) | (ppn_bits << 12);
    }

    fn set_flags(&mut self, flags: UniversalPTEFlag) {
        let la_flags = LAPTEFlags::from_universal(flags);
        self.0 = (self.0 & !0xfff) | (la_flags.bits() as u64);
    }

    fn clear(&mut self) {
        self.0 = 0;
    }

    fn remove_flags(&mut self, flags: UniversalPTEFlag) {
        let current = self.flags();
        self.set_flags(current & !flags);
    }

    fn add_flags(&mut self, flags: UniversalPTEFlag) {
        self.set_flags(self.flags() | flags);
    }
}
