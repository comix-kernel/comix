use crate::mm::address::Ppn;
use crate::mm::memory_space::mapping_area::MappingArea;
use crate::mm::page_table::{ActivePageTableInner, PageTableInner};
use alloc::vec::Vec;

// static KERNEL_SPACE: Mutex<Option<MemorySpace>> = Mutex::new(None);

/// Memory space structure representing an address space
pub struct MemorySpace {
    /// Page table associated with this memory space
    pub page_table: ActivePageTableInner,

    /// List of mapping areas in this memory space
    pub mapping_areas: Vec<MappingArea>,
}

impl MemorySpace {
    /// new a memory space
    pub fn new() -> Self {
        MemorySpace {
            page_table: ActivePageTableInner::new(),
            mapping_areas: Vec::new(),
        }
    }

    pub fn page_table(&self) -> &ActivePageTableInner {
        &self.page_table
    }

    pub fn page_table_mut(&mut self) -> &mut ActivePageTableInner {
        &mut self.page_table
    }

    pub fn root_ppn(&self) -> Ppn {
        self.page_table.root_ppn()
    }
}
