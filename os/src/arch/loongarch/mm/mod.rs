//! LoongArch64 内存管理模块（存根）
//!
//! 使用直接映射进行地址转换

mod page_table;
mod page_table_entry;

pub use page_table::PageTableInner;
pub use page_table_entry::PageTableEntry;

/// 虚拟地址起始 (LoongArch 直接映射窗口)
pub const VADDR_START: usize = 0x9000_0000_0000_0000;

/// 物理地址掩码
pub const PADDR_MASK: usize = 0x0000_ffff_ffff_ffff;

/// 虚拟地址转物理地址
pub const unsafe fn vaddr_to_paddr(vaddr: usize) -> usize {
    vaddr & PADDR_MASK
}

/// 物理地址转虚拟地址
pub const fn paddr_to_vaddr(paddr: usize) -> usize {
    paddr | VADDR_START
}
