use super::{ActivePageTableInner, PageSize, PageTableEntry, PagingResult, UniversalPTEFlag};
use crate::mm::address::{Paddr, Ppn, PpnRange, Vaddr, Vpn, VpnRange};

pub trait PageTableInner<T>
where
    T: PageTableEntry,
{
    const LEVELS: usize;
    const MAX_VA_BITS: usize;
    const MAX_PA_BITS: usize;

    fn tlb_flush(vpn: Vpn);
    fn tlb_flush_all();

    fn is_user_table(&self) -> bool;

    fn activate(ppn: Ppn);
    fn activating_table_ppn() -> Ppn;

    fn new() -> Self;
    fn from_ppn(ppn: Ppn) -> Self;
    fn new_as_kernel_table() -> Self;

    fn root_ppn(&self) -> Ppn;

    fn get_entry(&self, vpn: Vpn, level: usize) -> Option<(T, PageSize)>;

    fn translate(&self, vaddr: Vaddr) -> Option<Paddr>;

    fn map(
        &mut self,
        vpn: Vpn,
        ppn: Ppn,
        page_size: PageSize,
        flags: UniversalPTEFlag,
    ) -> PagingResult<()>;

    fn unmap(&mut self, vpn: Vpn) -> PagingResult<()>;

    fn mvmap(
        &mut self,
        vpn: Vpn,
        target_ppn: Ppn,
        page_size: PageSize,
        flags: UniversalPTEFlag,
    ) -> PagingResult<()>;

    fn update_flags(&mut self, vpn: Vpn, flags: UniversalPTEFlag) -> PagingResult<()>;

    fn walk(&self, vpn: Vpn) -> PagingResult<(Ppn, PageSize, UniversalPTEFlag)>;
}

pub struct PageTable {
    inner: ActivePageTableInner,
    // TODO:
    tracker: (),
}
