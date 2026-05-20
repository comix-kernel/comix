use crate::mm::address::{PA, Ppn, UsizeConvert, VA, Vpn};
use crate::mm::page_table::{
    PageSize, PageTableEntry as PageTableEntryTrait, PageTableInner as PageTableInnerTrait,
    PagingError, PagingResult, UniversalPTEFlag,
};

pub fn pa_to_va(pa: PA) -> VA {
    VA::from_usize(pa.as_usize() + super::constant::SV39_BOT_HALF_TOP)
}

pub unsafe fn va_to_pa(va: VA) -> PA {
    PA::from_usize(va.as_usize() - super::constant::SV39_BOT_HALF_TOP)
}

// ---- Mock PageTableEntry ----

#[derive(Debug, Clone, Copy)]
pub struct PageTableEntry {
    bits: u64,
}

impl PageTableEntryTrait for PageTableEntry {
    type Bits = u64;

    fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    fn to_bits(&self) -> u64 {
        self.bits
    }

    fn empty() -> Self {
        Self { bits: 0 }
    }

    fn new_leaf(ppn: Ppn, flags: UniversalPTEFlag) -> Self {
        Self {
            bits: ((ppn.as_usize() as u64) << 10) | (flags.bits() as u64),
        }
    }

    fn new_table(ppn: Ppn) -> Self {
        Self {
            bits: ((ppn.as_usize() as u64) << 10) | (UniversalPTEFlag::VALID.bits() as u64),
        }
    }

    fn is_valid(&self) -> bool {
        self.bits & (UniversalPTEFlag::VALID.bits() as u64) != 0
    }

    fn is_empty(&self) -> bool {
        self.bits == 0
    }

    fn ppn(&self) -> Ppn {
        Ppn::from_usize(((self.bits >> 10) & ((1u64 << 44) - 1)) as usize)
    }

    fn flags(&self) -> UniversalPTEFlag {
        UniversalPTEFlag::from_bits_truncate((self.bits & 0xff) as usize)
    }

    fn set_ppn(&mut self, ppn: Ppn) {
        let flags = self.bits & 0xff;
        self.bits = ((ppn.as_usize() as u64) << 10) | flags;
    }

    fn set_flags(&mut self, flags: UniversalPTEFlag) {
        let ppn_bits = self.bits & !0xff;
        self.bits = ppn_bits | (flags.bits() as u64);
    }

    fn clear(&mut self) {
        self.bits = 0;
    }

    fn remove_flags(&mut self, flags: UniversalPTEFlag) {
        let current = UniversalPTEFlag::from_bits_truncate((self.bits & 0xff) as usize);
        let updated = current.difference(flags);
        self.bits = (self.bits & !0xff) | (updated.bits() as u64);
    }

    fn add_flags(&mut self, flags: UniversalPTEFlag) {
        let current = UniversalPTEFlag::from_bits_truncate((self.bits & 0xff) as usize);
        let updated = current.union(flags);
        self.bits = (self.bits & !0xff) | (updated.bits() as u64);
    }
}

// ---- Mock PageTableInner ----

#[derive(Debug)]
pub struct PageTableInner {
    root: Ppn,
    is_user: bool,
}

impl PageTableInnerTrait<PageTableEntry> for PageTableInner {
    const LEVELS: usize = 3;
    const MAX_VA_BITS: usize = 39;
    const MAX_PA_BITS: usize = 56;

    fn tlb_flush(_vpn: Vpn) {}

    fn tlb_flush_all() {}

    fn is_user_table(&self) -> bool {
        self.is_user
    }

    fn activate(_ppn: Ppn) {}

    fn activating_table_ppn() -> Ppn {
        Ppn::from_usize(0)
    }

    fn new() -> Self {
        Self {
            root: Ppn::from_usize(0x80000),
            is_user: true,
        }
    }

    fn from_ppn(ppn: Ppn) -> Self {
        Self {
            root: ppn,
            is_user: true,
        }
    }

    fn new_as_kernel_table() -> Self {
        Self {
            root: Ppn::from_usize(0x80000),
            is_user: false,
        }
    }

    fn root_ppn(&self) -> Ppn {
        self.root
    }

    fn get_entry(&self, _vpn: Vpn, _level: usize) -> Option<(PageTableEntry, PageSize)> {
        None
    }

    fn translate(&self, _vaddr: VA) -> Option<PA> {
        None
    }

    fn map(
        &mut self,
        _vpn: Vpn,
        _ppn: Ppn,
        _page_size: PageSize,
        _flags: UniversalPTEFlag,
    ) -> PagingResult<()> {
        Ok(())
    }

    fn unmap(&mut self, _vpn: Vpn) -> PagingResult<()> {
        Ok(())
    }

    fn mvmap(
        &mut self,
        _vpn: Vpn,
        target_ppn: Ppn,
        _page_size: PageSize,
        _flags: UniversalPTEFlag,
    ) -> PagingResult<()> {
        self.root = target_ppn;
        Ok(())
    }

    fn update_flags(&mut self, _vpn: Vpn, _flags: UniversalPTEFlag) -> PagingResult<()> {
        Ok(())
    }

    fn walk(&self, _vpn: Vpn) -> PagingResult<(Ppn, PageSize, UniversalPTEFlag)> {
        Err(PagingError::NotMapped)
    }
}

// Batch methods (non-trait, architecture-specific helpers)

impl PageTableInner {
    pub fn map_with_batch(
        &mut self,
        vpn: Vpn,
        ppn: Ppn,
        page_size: PageSize,
        flags: UniversalPTEFlag,
        _batch: Option<&mut TlbBatchContext>,
    ) -> PagingResult<()> {
        <Self as PageTableInnerTrait<PageTableEntry>>::map(self, vpn, ppn, page_size, flags)
    }

    pub fn unmap_with_batch(
        &mut self,
        vpn: Vpn,
        _batch: Option<&mut TlbBatchContext>,
    ) -> PagingResult<()> {
        <Self as PageTableInnerTrait<PageTableEntry>>::unmap(self, vpn)
    }

    pub fn update_flags_with_batch(
        &mut self,
        vpn: Vpn,
        flags: UniversalPTEFlag,
        _batch: Option<&mut TlbBatchContext>,
    ) -> PagingResult<()> {
        <Self as PageTableInnerTrait<PageTableEntry>>::update_flags(self, vpn, flags)
    }
}

// ---- TlbBatchContext ----

pub struct TlbBatchContext {
    enabled: bool,
}

impl TlbBatchContext {
    pub fn new() -> Self {
        Self { enabled: false }
    }

    pub fn execute<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        let mut ctx = Self::new();
        let result = f(&mut ctx);
        drop(ctx);
        result
    }

    pub fn flush(&mut self) {}
}

impl Drop for TlbBatchContext {
    fn drop(&mut self) {}
}
