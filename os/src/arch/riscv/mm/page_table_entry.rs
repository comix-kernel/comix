use crate::mm::address::{Ppn, UsizeConvert};
use crate::mm::page_table::PageTableEntry as PageTableEntryTrait;
use crate::mm::page_table::{UniversalConvertableFlag, UniversalPTEFlag};

// 使用 bitflags 宏定义 SV39 页表项的标志位
bitflags::bitflags! {
    pub struct SV39PTEFlags: usize {
        const VALID = 1 << 0;       // 指示页表项是否有效
        const READ = 1 << 1;        // 指示页面是否可读 (R)
        const WRITE = 1 << 2;       // 指示页面是否可写 (W)
        const EXECUTE = 1 << 3;     // 指示页面是否可执行 (X)
        const USER = 1 << 4;        // 指示页面是否允许用户模式访问 (U)
        const GLOBAL = 1 << 5;      // 指示页面是否为全局页 (G)
        const ACCESSED = 1 << 6;    // 指示页面是否已被访问 (A)
        const DIRTY = 1 << 7;       // 指示页面是否已被写入 (D)
        const _RESERVED = 1 << 8;   // 保留位，供将来使用 (根据 SV39 规范)
    }
}

/*
 * SV39 页表项 (PTE) 格式:
 * ------------------------------------------------
 * | 位数  | 描述                                |
 * ------------------------------------------------
 * | 0-7   | 标志位 (有效、读、写、执行、用户、      |
 * |       | 全局、已访问、脏)                     |
 * ------------------------------------------------
 * | 8-9   | 保留 (必须为零)                      |
 * ------------------------------------------------
 * | 10-53 | 物理页号 (PPN)                      |
 * ------------------------------------------------
 * | 54-63 | 保留 (必须为零)                     |
 * ------------------------------------------------
 */

const SV39_PTE_FLAG_MASK: usize = 0xff; // SV39 PTE 标志位，占用低 8 位
const SV39_PTE_PPN_OFFSET: usize = 10; // 物理页号 (PPN) 从第 10 位开始
const SV39_PTE_PPN_MASK: u64 = 0x0000_ffff_ffff_fc00; // PPN 掩码，覆盖位 10-53

// 实现通用标志位与 SV39 特定标志位之间的转换
impl UniversalConvertableFlag for SV39PTEFlags {
    // 从通用标志位转换为 SV39 标志位
    fn from_universal(flag: UniversalPTEFlag) -> Self {
        // 仅保留 SV39 定义的低 8 位标志
        Self::from_bits(flag.bits() & SV39_PTE_FLAG_MASK).unwrap()
    }

    // 将 SV39 标志位转换为通用标志位
    fn to_universal(&self) -> UniversalPTEFlag {
        UniversalPTEFlag::from_bits(self.bits() & SV39_PTE_FLAG_MASK).unwrap()
    }
}

// 页表项结构体，内部存储为 64 位整数
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PageTableEntry(u64);

// 实现页表项的通用特性 (PageTableEntryTrait)
impl PageTableEntryTrait for PageTableEntry {
    type Bits = u64; // 底层位类型

    // 从原始位创建页表项
    fn from_bits(bits: Self::Bits) -> Self {
        PageTableEntry(bits)
    }

    // 获取页表项的原始位
    fn to_bits(&self) -> Self::Bits {
        self.0
    }

    // 创建一个空的页表项 (所有位为零)
    fn empty() -> Self {
        PageTableEntry(0)
    }

    // 创建一个新的叶子节点 (指向实际页面) 的页表项
    fn new_leaf(ppn: Ppn, flags: UniversalPTEFlag) -> Self {
        let ppn_bits: u64 = ppn.as_usize() as u64;
        let sv39_flags = SV39PTEFlags::from_universal(flags);
        // PPN 左移 10 位，并与标志位进行位或操作
        PageTableEntry((ppn_bits << SV39_PTE_PPN_OFFSET) | (sv39_flags.bits() as u64))
    }

    // 创建一个新的表节点 (指向下一级页表) 的页表项
    fn new_table(ppn: Ppn) -> Self {
        let ppn_bits: u64 = ppn.as_usize() as u64;
        // 表节点只需设置 VALID 标志位
        let sv39_flags = SV39PTEFlags::VALID;
        PageTableEntry((ppn_bits << SV39_PTE_PPN_OFFSET) | (sv39_flags.bits() as u64))
    }

    // 检查页表项是否有效 (即最低位是否设置)
    fn is_valid(&self) -> bool {
        (self.0 & SV39PTEFlags::VALID.bits() as u64) != 0
    }

    // 检查页表项是否代表巨页 (Huge Page)
    fn is_huge(&self) -> bool {
        // 在 SV39 中，我们无法仅凭 PTE 自身直接判断巨页。
        // 如果 PTE 带有 R/W/X 权限，则被视为叶子节点 (可能是巨页或普通页)，
        // 否则它是一个指向下一级页表的中间节点。
        let sv39_flags =
            SV39PTEFlags::from_bits((self.0 & SV39_PTE_FLAG_MASK as u64) as usize).unwrap();
        // 检查是否与 R 或 X 或 W 权限相交
        sv39_flags.intersects(
            SV39PTEFlags::union(SV39PTEFlags::READ, SV39PTEFlags::EXECUTE)
                .union(SV39PTEFlags::WRITE),
        )
    }

    // 检查页表项是否为空 (所有位为零)
    fn is_empty(&self) -> bool {
        self.0 == 0
    }

    // 获取页表项中的物理页号 (PPN)
    fn ppn(&self) -> Ppn {
        // 提取 PPN 位，并右移 10 位
        let ppn = (self.0 & SV39_PTE_PPN_MASK) >> SV39_PTE_PPN_OFFSET;
        Ppn::from_usize(ppn as usize)
    }

    // 获取页表项中的标志位 (转换为通用格式)
    fn flags(&self) -> UniversalPTEFlag {
        // 提取低 8 位标志
        let sv39_flags =
            SV39PTEFlags::from_bits((self.0 & SV39_PTE_FLAG_MASK as u64) as usize).unwrap();
        sv39_flags.to_universal()
    }

    // 设置页表项中的物理页号 (PPN)
    fn set_ppn(&mut self, ppn: Ppn) {
        let ppn_bits = ppn.as_usize() as u64;
        // 清除旧的 PPN 位，并设置新的 PPN 位
        self.0 = (self.0 & !SV39_PTE_PPN_MASK) | (ppn_bits << SV39_PTE_PPN_OFFSET);
    }

    // 设置页表项中的标志位
    fn set_flags(&mut self, flags: UniversalPTEFlag) {
        let sv39_flags = SV39PTEFlags::from_universal(flags);
        // 清除旧的标志位，并设置新的标志位
        self.0 = (self.0 & !(SV39_PTE_FLAG_MASK as u64)) | (sv39_flags.bits() as u64);
    }

    // 清空页表项 (设置为零)
    fn clear(&mut self) {
        self.0 = 0;
    }

    // 移除指定的标志位 (current_flags & !flags)
    fn remove_flags(&mut self, flags: UniversalPTEFlag) {
        let current_flags = self.flags();
        let new_flags = current_flags & !flags;
        self.set_flags(new_flags);
    }

    // 添加指定的标志位 (current_flags | flags)
    fn add_flags(&mut self, flags: UniversalPTEFlag) {
        self.set_flags(self.flags() | flags);
    }
}
