//! LoongArch64 页表项定义
//!
//! 根据《LoongArch 64-bit ISA Reference Manual》第 5.4 节和第 7.5 节实现。
//!
//! # PTE 格式 (TLBELO)
//!
//! | 位 | 字段 | 说明 |
//! |----|------|------|
//! | 0 | V | Valid/Accessed - 有效/访问位 |
//! | 1 | D | Dirty - 脏位 |
//! | 3:2 | PLV | Privilege Level (0=内核, 3=用户) |
//! | 5:4 | MAT | Memory Access Type |
//! | 6 | G | Global - 全局位 |
//! | 7 | P | Present - 软件 present 位 |
//! | 8 | W | Write - 软件写权限位 |
//! | 9 | M | Modified - 软件 modified 位 |
//! | 10 | PROTNONE | 软件 PROT_NONE 位 |
//! | 11 | SPECIAL | 软件 special 位 |
//! | 47:12 | PPN | 物理页号 (PALEN-1:12) |
//! | 60:48 | 保留 | 必须为 0 |
//! | 61 | NR | Non-Readable (0=可读，1=不可读) |
//! | 62 | NX | Non-eXecutable (0=可执行，1=不可执行) |
//! | 63 | RPLV | Restricted PLV |

use crate::mm::address::{Ppn, UsizeConvert};
use crate::mm::page_table::PageTableEntry as PageTableEntryTrait;
use crate::mm::page_table::{UniversalConvertableFlag, UniversalPTEFlag};

bitflags::bitflags! {
    /// LoongArch 页表项标志位
    ///
    /// 基于 TLBELO0/TLBELO1 寄存器格式定义。
    /// 注意：LoongArch 使用 NR/NX 表示不可读/不可执行（反逻辑）
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LAPTEFlags: u64 {
        /// 有效/访问位 (bit 0)
        const VALID = 1 << 0;

        /// 脏位 (bit 1)
        const DIRTY = 1 << 1;

        /// 特权级位 0 (bit 2)
        const PLV_BIT0 = 1 << 2;
        /// 特权级位 1 (bit 3)
        const PLV_BIT1 = 1 << 3;

        /// 特权级 0 - 内核态 (PLV bits 2-3 = 0b00)
        const PLV0 = 0;
        /// 特权级 3 - 用户态 (PLV bits 2-3 = 0b11)
        const PLV3 = (1 << 2) | (1 << 3);

        /// 内存访问类型位 0 (bit 4)
        const MAT_BIT0 = 1 << 4;
        /// 内存访问类型位 1 (bit 5)
        const MAT_BIT1 = 1 << 5;

        /// 内存访问类型 - Coherent Cached (MAT = 0b01)
        const MAT_CC = 1 << 4;
        /// 内存访问类型 - Strongly-ordered UnCached (MAT = 0b00)
        const MAT_SUC = 0;
        /// 内存访问类型 - Weakly-ordered UnCached (MAT = 0b10)
        const MAT_WUC = 1 << 5;

        /// 全局位 (bit 6)
        const GLOBAL = 1 << 6;

        /// 软件 present 位 (bit 7)
        const PRESENT = 1 << 7;

        /// 软件写权限位 (bit 8)
        const WRITE = 1 << 8;

        /// 软件 modified 位 (bit 9)
        const MODIFIED = 1 << 9;

        /// 软件 PROT_NONE 位 (bit 10)
        const PROTNONE = 1 << 10;

        /// 软件 special 位 (bit 11)
        const SPECIAL = 1 << 11;

        /// 不可读位 (bit 61) - 0=可读，1=不可读
        /// 仅 LA64 支持
        const NR = 1 << 61;

        /// 不可执行位 (bit 62) - 0=可执行，1=不可执行
        /// 仅 LA64 支持
        const NX = 1 << 62;

        /// 受限特权级 (bit 63)
        /// RPLV=0: 当前 PLV <= 页表 PLV 时可访问
        /// RPLV=1: 当前 PLV == 页表 PLV 时可访问
        const RPLV = 1 << 63;
    }
}

/// PLV 字段掩码 (bits 2-3)
const PLV_MASK: u64 = 0b11 << 2;

impl LAPTEFlags {
    /// 获取 PLV 字段值 (0-3)
    #[inline]
    pub fn plv(&self) -> u8 {
        ((self.bits() & PLV_MASK) >> 2) as u8
    }

    /// 检查是否为用户态页面 (PLV=3)
    #[inline]
    pub fn is_user(&self) -> bool {
        self.plv() == 3
    }

    /// 检查是否可读 (NR=0)
    #[inline]
    pub fn is_readable(&self) -> bool {
        !self.contains(LAPTEFlags::NR)
    }

    /// 检查是否可写 (D=1)
    #[inline]
    pub fn is_writable(&self) -> bool {
        self.contains(LAPTEFlags::DIRTY)
    }

    /// 检查是否可执行 (NX=0)
    #[inline]
    pub fn is_executable(&self) -> bool {
        !self.contains(LAPTEFlags::NX)
    }
}

impl UniversalConvertableFlag for LAPTEFlags {
    /// 从通用标志位转换为 LoongArch 标志位
    ///
    /// # 翻译规则
    /// - VALID → VALID + MAT_CC
    /// - READABLE → 不设置 NR (NR=0 表示可读)
    /// - WRITEABLE → DIRTY (D 位控制写权限)
    /// - EXECUTABLE → 不设置 NX (NX=0 表示可执行)
    /// - USER_ACCESSIBLE → PLV3
    /// - GLOBAL → GLOBAL
    /// - DIRTY → DIRTY
    fn from_universal(flag: UniversalPTEFlag) -> Self {
        let mut result = LAPTEFlags::empty();

        if flag.contains(UniversalPTEFlag::VALID) {
            // 标记软件 present，并启用一致性缓存。
            // 这里同时设置 VALID 位（不做 accessed/dirty 跟踪）。
            result |= LAPTEFlags::PRESENT | LAPTEFlags::VALID | LAPTEFlags::MAT_CC;
        }

        // READABLE: 不设置 NR 位（默认 NR=0 表示可读）
        // 只有明确不可读时才设置 NR
        if !flag.contains(UniversalPTEFlag::READABLE) {
            result |= LAPTEFlags::NR;
        }

        if flag.contains(UniversalPTEFlag::WRITEABLE) {
            // LoongArch Linux 软件约定：WRITE 控制写权限，DIRTY 用于脏页标记。
            // 为简化实现，这里对可写页同时置位 WRITE + DIRTY。
            result |= LAPTEFlags::WRITE | LAPTEFlags::DIRTY;
        }

        // EXECUTABLE: 不设置 NX 位（默认 NX=0 表示可执行）
        // 只有明确不可执行时才设置 NX
        if !flag.contains(UniversalPTEFlag::EXECUTABLE) {
            result |= LAPTEFlags::NX;
        }

        if flag.contains(UniversalPTEFlag::USER_ACCESSIBLE) {
            result |= LAPTEFlags::PLV3;
        }
        // 内核态时 PLV=0，不需要设置任何位

        if flag.contains(UniversalPTEFlag::DIRTY) {
            result |= LAPTEFlags::DIRTY;
        }

        if flag.contains(UniversalPTEFlag::GLOBAL) {
            result |= LAPTEFlags::GLOBAL;
        }

        result
    }

    /// 将 LoongArch 标志位转换为通用标志位
    ///
    /// # 翻译规则
    /// - VALID → VALID
    /// - NR=0 → READABLE
    /// - DIRTY → WRITEABLE + DIRTY
    /// - NX=0 → EXECUTABLE
    /// - PLV=3 → USER_ACCESSIBLE
    /// - GLOBAL → GLOBAL
    fn to_universal(&self) -> UniversalPTEFlag {
        let mut result = UniversalPTEFlag::empty();

        if self.contains(LAPTEFlags::PRESENT) {
            result |= UniversalPTEFlag::VALID;
        }

        // NR=0 表示可读
        if !self.contains(LAPTEFlags::NR) {
            result |= UniversalPTEFlag::READABLE;
        }

        if self.contains(LAPTEFlags::WRITE) {
            result |= UniversalPTEFlag::WRITEABLE;
        }
        if self.contains(LAPTEFlags::DIRTY) {
            result |= UniversalPTEFlag::DIRTY;
        }

        // NX=0 表示可执行
        if !self.contains(LAPTEFlags::NX) {
            result |= UniversalPTEFlag::EXECUTABLE;
        }

        // PLV=3 表示用户态可访问
        if self.is_user() {
            result |= UniversalPTEFlag::USER_ACCESSIBLE;
        }

        if self.contains(LAPTEFlags::GLOBAL) {
            result |= UniversalPTEFlag::GLOBAL;
        }

        result
    }
}

/// PPN 在 PTE 中的偏移量 (bit 12 开始)
const LA64_PTE_PPN_OFFSET: u32 = 12;

/// PPN 掩码 (bits 12-47，支持 48 位物理地址)
const LA64_PTE_PPN_MASK: u64 = 0x0000_FFFF_FFFF_F000;

    /// 标志位掩码 (bits 0-11 + 61-63)
    const LA64_PTE_FLAG_MASK_LOW: u64 = 0x0FFF; // bits 0-11
    const LA64_PTE_FLAG_MASK_HIGH: u64 = 0xE000_0000_0000_0000; // bits 61-63

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

    /// 创建叶子节点（实际页面映射）
    fn new_leaf(ppn: Ppn, flags: UniversalPTEFlag) -> Self {
        let ppn_bits: u64 = ppn.as_usize() as u64;
        let la_flags = LAPTEFlags::from_universal(flags);
        // PPN 左移 12 位，与标志位进行位或操作
        PageTableEntry((ppn_bits << LA64_PTE_PPN_OFFSET) | la_flags.bits())
    }

    /// 创建表节点（指向下一级页表）
    ///
    /// LoongArch 目录项不设置 V 位，只存储物理地址。
    /// LDDIR 指令会直接使用目录项中的地址作为下一级页表基址。
    fn new_table(ppn: Ppn) -> Self {
        let ppn_bits: u64 = ppn.as_usize() as u64;
        // 目录项只存储物理地址，不设置任何标志位
        PageTableEntry(ppn_bits << LA64_PTE_PPN_OFFSET)
    }

    fn is_valid(&self) -> bool {
        (self.0 & LAPTEFlags::PRESENT.bits()) != 0
    }

    /// 检查是否为巨页（Huge Page）
    ///
    /// LoongArch 中，需要通过页表级别来判断巨页，
    /// 单独从 PTE 无法直接判断。这里返回 false 作为保守实现。
    fn is_huge(&self) -> bool {
        // LoongArch 不像 RISC-V 那样通过 R/W/X 位区分叶子节点
        // 巨页检测需要结合页表级别信息
        false
    }

    fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// 获取物理页号 (PPN)
    fn ppn(&self) -> Ppn {
        let ppn = (self.0 & LA64_PTE_PPN_MASK) >> LA64_PTE_PPN_OFFSET;
        Ppn::from_usize(ppn as usize)
    }

    /// 获取标志位（转换为通用格式）
    fn flags(&self) -> UniversalPTEFlag {
        let flags_bits = (self.0 & LA64_PTE_FLAG_MASK_LOW) | (self.0 & LA64_PTE_FLAG_MASK_HIGH);
        let la_flags = LAPTEFlags::from_bits_truncate(flags_bits);
        la_flags.to_universal()
    }

    fn set_ppn(&mut self, ppn: Ppn) {
        let ppn_bits = (ppn.as_usize() as u64) << LA64_PTE_PPN_OFFSET;
        // 清除旧 PPN，保留标志位
        self.0 = (self.0 & !LA64_PTE_PPN_MASK) | ppn_bits;
    }

    fn set_flags(&mut self, flags: UniversalPTEFlag) {
        let la_flags = LAPTEFlags::from_universal(flags);
        // 清除旧标志位，保留 PPN
        self.0 = (self.0 & LA64_PTE_PPN_MASK)
            | (la_flags.bits() & (LA64_PTE_FLAG_MASK_LOW | LA64_PTE_FLAG_MASK_HIGH));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kassert, test_case};

    // 1. 标志位转换测试 - 内核读写
    test_case!(test_flag_conversion_kernel_rw, {
        let universal = UniversalPTEFlag::kernel_rw();
        let la_flags = LAPTEFlags::from_universal(universal);

        // 应该有 VALID, DIRTY, MAT_CC，不应该有 NR
        kassert!(la_flags.contains(LAPTEFlags::VALID));
        kassert!(la_flags.contains(LAPTEFlags::DIRTY));
        kassert!(la_flags.contains(LAPTEFlags::MAT_CC));
        kassert!(!la_flags.contains(LAPTEFlags::NR)); // 可读

        // PLV 应该为 0（内核态）
        kassert!(!la_flags.is_user());
    });

    // 2. 标志位转换测试 - 用户读执行
    test_case!(test_flag_conversion_user_rx, {
        let universal = UniversalPTEFlag::user_rx();
        let la_flags = LAPTEFlags::from_universal(universal);

        // 应该有 VALID, PLV3, MAT_CC
        kassert!(la_flags.contains(LAPTEFlags::VALID));
        kassert!(la_flags.is_user());
        kassert!(!la_flags.contains(LAPTEFlags::NR)); // 可读
        kassert!(!la_flags.contains(LAPTEFlags::NX)); // 可执行
        kassert!(!la_flags.contains(LAPTEFlags::DIRTY)); // 不可写
    });

    // 3. 往返转换测试
    test_case!(test_roundtrip_conversion, {
        let original = UniversalPTEFlag::user_rw();
        let la_flags = LAPTEFlags::from_universal(original);
        let converted_back = la_flags.to_universal();

        // 应该能够往返转换（忽略 ACCESSED，因为 LoongArch 没有）
        kassert!(converted_back.contains(UniversalPTEFlag::VALID));
        kassert!(converted_back.contains(UniversalPTEFlag::READABLE));
        kassert!(converted_back.contains(UniversalPTEFlag::WRITEABLE));
        kassert!(converted_back.contains(UniversalPTEFlag::USER_ACCESSIBLE));
    });
}
