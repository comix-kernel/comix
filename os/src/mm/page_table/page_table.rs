use super::{ActivePageTableInner, PageResult, PageSize, PageTableEntry, UniversalPTEFlag};
use crate::mm::address::{Paddr, PaddrRange, Vaddr, VaddrRange};

pub trait PageTableInner<T>
where
    T: PageTableEntry,
{
    const LEVELS: usize;
    const MAX_VA_BITS: usize;
    const MAX_PA_BITS: usize;

    fn tlb_flush(vaddr: Vaddr);
    fn tlb_flush_range(start_vaddr: Vaddr, size: usize);
    fn tlb_flush_all();

    fn is_user_table() -> bool;

    fn activate(paddr: Paddr);
    fn activating_table_paddr() -> Paddr;

    fn new() -> Self;
    fn from_paddr(paddr: Paddr) -> Self;
    fn new_as_kernel_table() -> Self;

    fn root_paddr(&self) -> Paddr;

    fn get_entry(&self, vaddr: Vaddr, level: usize) -> Option<(T, PageSize)>;

    fn translate(&self, vaddr: Vaddr) -> Option<Paddr>;

    fn map(
        &mut self,
        vaddr: Vaddr,
        paddr: Paddr,
        page_size: PageSize,
        flags: UniversalPTEFlag,
    ) -> PageResult<()>;

    fn unmap(&mut self, vaddr: Vaddr) -> PageResult<(Paddr, PageSize)>;

    fn mvmap(
        &mut self,
        vaddr: Vaddr,
        target_paddr: Paddr,
        page_size: PageSize,
        flags: UniversalPTEFlag,
    ) -> PageResult<(Paddr, PageSize)>;

    fn update_flags(&mut self, vaddr: Vaddr, flags: UniversalPTEFlag) -> PageResult<()>;

    fn map_range(
        &mut self,
        vaddr_range: VaddrRange,
        paddr_range: PaddrRange,
        flags: UniversalPTEFlag,
    ) -> PageResult<()>;

    fn unmap_range(&mut self, vaddr_range: VaddrRange) -> PageResult<PaddrRange>;

    fn mvmap_range(
        &mut self,
        vaddr_range: VaddrRange,
        target_paddr_range: PaddrRange,
        flags: UniversalPTEFlag,
    ) -> PageResult<PaddrRange>;

    fn update_flags_range(
        &mut self,
        vaddr_range: VaddrRange,
        flags: UniversalPTEFlag,
    ) -> PageResult<()>;

    fn walk(&self, vaddr: Vaddr) -> PageResult<(Paddr, PageSize, UniversalPTEFlag)>;
}

pub struct PageTable {
    inner: ActivePageTableInner,
    // TODO: 
    tracker: (),
}
