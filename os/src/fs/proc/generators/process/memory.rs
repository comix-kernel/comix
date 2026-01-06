use crate::{
    config::PAGE_SIZE,
    mm::memory_space::MemorySpace,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct ProcMemStats {
    pub vm_size_bytes: usize,
    pub rss_bytes: usize,
    pub text_bytes: usize,
    pub rodata_bytes: usize,
    pub data_bytes: usize,
    pub bss_bytes: usize,
    pub heap_bytes: usize,
    pub stack_bytes: usize,
    pub mmap_bytes: usize,
}

impl ProcMemStats {
    pub fn vm_size_kb(&self) -> usize {
        self.vm_size_bytes / 1024
    }
    pub fn rss_kb(&self) -> usize {
        self.rss_bytes / 1024
    }
}

pub fn collect_user_vm_stats(space: &MemorySpace) -> ProcMemStats {
    use crate::mm::memory_space::mapping_area::AreaType;

    let mut s = ProcMemStats::default();

    for area in space.areas().iter() {
        let at = area.area_type();
        let is_user = matches!(
            at,
            AreaType::UserText
                | AreaType::UserRodata
                | AreaType::UserData
                | AreaType::UserBss
                | AreaType::UserStack
                | AreaType::UserHeap
                | AreaType::UserMmap
        );
        if !is_user {
            continue;
        }

        let pages = area.vpn_range().len();
        let bytes = pages * PAGE_SIZE;
        s.vm_size_bytes = s.vm_size_bytes.saturating_add(bytes);

        // Resident pages (committed frames). For now this is typically equal to `pages` because we
        // eagerly map user VMAs, but keep it explicit for future lazy paging/COW work.
        let rss_pages = area.mapped_pages();
        s.rss_bytes = s.rss_bytes.saturating_add(rss_pages * PAGE_SIZE);

        match at {
            AreaType::UserText => s.text_bytes = s.text_bytes.saturating_add(bytes),
            AreaType::UserRodata => s.rodata_bytes = s.rodata_bytes.saturating_add(bytes),
            AreaType::UserData => s.data_bytes = s.data_bytes.saturating_add(bytes),
            AreaType::UserBss => s.bss_bytes = s.bss_bytes.saturating_add(bytes),
            AreaType::UserHeap => s.heap_bytes = s.heap_bytes.saturating_add(bytes),
            AreaType::UserStack => s.stack_bytes = s.stack_bytes.saturating_add(bytes),
            AreaType::UserMmap => s.mmap_bytes = s.mmap_bytes.saturating_add(bytes),
            _ => {}
        }
    }

    s
}

