use alloc::vec::Vec;
use crate::mm::page_table::ActivePageTableInner;
use crate::mm::memory_space::mapping_area::MappingArea;

// TODO: Refactor with proper synchronization
static mut KERNEL_SPACE: Option<MemorySpace> = None;

pub struct MemorySpace {
    pub page_table: ActivePageTableInner,
    pub mapping_areas: Vec<MappingArea>,
}

