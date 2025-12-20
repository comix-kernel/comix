//! LoongArch64 页表管理（存根）

use super::PageTableEntry;
use crate::mm::address::{Paddr, Ppn, UsizeConvert, Vaddr, Vpn};
use crate::mm::frame_allocator::FrameTracker;
use crate::mm::page_table::{
    PageSize, PageTableEntry as PageTableEntryTrait, PageTableInner as PageTableInnerTrait,
    PagingError, PagingResult, UniversalPTEFlag,
};
use alloc::vec::Vec;

/// 页表内部结构
#[derive(Debug)]
pub struct PageTableInner {
    root_ppn: Ppn,
    frames: Vec<FrameTracker>,
    is_user: bool,
}

impl PageTableInnerTrait<PageTableEntry> for PageTableInner {
    const LEVELS: usize = 4; // LoongArch 4 级页表
    const MAX_VA_BITS: usize = 48;
    const MAX_PA_BITS: usize = 48;

    fn tlb_flush(_vpn: Vpn) {
        // TODO: 实现 TLB 刷新
    }

    fn tlb_flush_all() {
        // TODO: 实现全局 TLB 刷新
    }

    fn is_user_table(&self) -> bool {
        self.is_user
    }

    fn activate(_ppn: Ppn) {
        // TODO: 激活页表
    }

    fn activating_table_ppn() -> Ppn {
        // TODO: 返回当前激活的页表 PPN
        Ppn::from_usize(0)
    }

    fn new() -> Self {
        Self {
            root_ppn: Ppn::from_usize(0),
            frames: Vec::new(),
            is_user: true,
        }
    }

    fn from_ppn(ppn: Ppn) -> Self {
        Self {
            root_ppn: ppn,
            frames: Vec::new(),
            is_user: false,
        }
    }

    fn new_as_kernel_table() -> Self {
        Self {
            root_ppn: Ppn::from_usize(0),
            frames: Vec::new(),
            is_user: false,
        }
    }

    fn root_ppn(&self) -> Ppn {
        self.root_ppn
    }

    fn get_entry(&self, _vpn: Vpn, _level: usize) -> Option<(PageTableEntry, PageSize)> {
        // TODO: 实现
        None
    }

    fn translate(&self, _vaddr: Vaddr) -> Option<Paddr> {
        // TODO: 实现
        None
    }

    fn map(
        &mut self,
        _vpn: Vpn,
        _ppn: Ppn,
        _page_size: PageSize,
        _flags: UniversalPTEFlag,
    ) -> PagingResult<()> {
        // TODO: 实现
        Ok(())
    }

    fn unmap(&mut self, _vpn: Vpn) -> PagingResult<()> {
        // TODO: 实现
        Ok(())
    }

    fn mvmap(
        &mut self,
        _vpn: Vpn,
        _target_ppn: Ppn,
        _page_size: PageSize,
        _flags: UniversalPTEFlag,
    ) -> PagingResult<()> {
        // TODO: 实现
        Ok(())
    }

    fn update_flags(&mut self, _vpn: Vpn, _flags: UniversalPTEFlag) -> PagingResult<()> {
        // TODO: 实现
        Ok(())
    }

    fn walk(&self, _vpn: Vpn) -> PagingResult<(Ppn, PageSize, UniversalPTEFlag)> {
        // TODO: 实现
        Err(PagingError::NotMapped)
    }
}
