use alloc::collections::btree_map::BTreeMap;
use core::cmp::min;

use crate::mm::address::{ConvertablePaddr, PageNum, UsizeConvert, Vpn, VpnRange, Ppn};
use crate::mm::frame_allocator::{TrackedFrames, alloc_frame};
use crate::mm::page_table::{
    self, ActivePageTableInner, PageSize, PageTableInner, UniversalPTEFlag,
};

/// Mapping stretagy type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapType {
    /// Identical mapped (virtual address equals physical address)
    Identical,
    /// Frame mapped (allocated from frame allocator)
    Framed,
}

/// Type of the memory area
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaType {
    KernelText,   // Kernel code segment
    KernelRodata, // Kernel read-only data segment
    KernelData,   // Kernel data segment
    KernelBss,    // Kernel BSS segment
    KernelHeap,   // Kernel heap
    KernelMmio,   // Kernel memory-mapped I/O
    UserText,     // User code segment
    UserRodata,   // User read-only data segment
    UserData,     // User data segment
    UserBss,      // User BSS segment
    UserStack,    // User stack
    UserHeap,     // User heap
}

/// A memory mapping area in a memory space
pub struct MappingArea {
    /// Virtual page number range of this mapping area
    ///
    /// Attention!
    ///
    /// don't change it after created,
    /// don't use it to map or unmap pages
    vpn_range: VpnRange,

    /// Type of this mapping area
    area_type: AreaType,

    /// Mapping strategy type
    map_type: MapType,

    /// Permission permission for this mapping area (use UniversalPTEFlag for perfermance)
    permission: UniversalPTEFlag,

    /// Tracked frames for framed mapping area
    frames: BTreeMap<Vpn, TrackedFrames>,
}

impl MappingArea {
    pub fn vpn_range(&self) -> VpnRange {
        self.vpn_range
    }

    pub fn permission(&self) -> UniversalPTEFlag {
        self.permission
    }

    pub fn map_type(&self) -> MapType {
        self.map_type
    }

    pub fn area_type(&self) -> AreaType {
        self.area_type
    }

    pub fn new(
        vpn_range: VpnRange,
        area_type: AreaType,
        map_type: MapType,
        permission: UniversalPTEFlag,
    ) -> Self {
        MappingArea {
            vpn_range,
            area_type,
            map_type,
            permission,
            frames: BTreeMap::new(),
        }
    }

    /// Map a single virtual page to a physical page
    pub fn map_one(&mut self, page_table: &mut ActivePageTableInner, vpn: Vpn) -> Result<(), &'static str> {
        let ppn = match self.map_type {
            MapType::Identical => {
                // For identical mapping, vpn equals ppn
                use crate::mm::address::Ppn;
                Ppn::from_usize(vpn.as_usize())
            }
            MapType::Framed => {
                // Allocate a new frame
                let frame = alloc_frame().expect("failed to allocate frame");
                let ppn = frame.ppn();
                self.frames.insert(vpn, TrackedFrames::Single(frame));
                ppn
            }
        };

        page_table
            .map(vpn, ppn, PageSize::Size4K, self.permission)
            .expect("failed to map page");
        Ok(())
    }

    /// Map all pages in this mapping area
    pub fn map(&mut self, page_table: &mut ActivePageTableInner) -> Result<(), &'static str> {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn)?;
        }
        Ok(())
    }

    /// Unmap a single virtual page
    pub fn unmap_one(&mut self, page_table: &mut ActivePageTableInner, vpn: Vpn) -> Result<(), &'static str> {
        page_table.unmap(vpn).expect("failed to unmap page");

        // For framed mapping, remove the frame tracker
        if self.map_type == MapType::Framed {
            self.frames.remove(&vpn);
        }
        Ok(())
    }

    /// Unmap all pages in this mapping area
    pub fn unmap(&mut self, page_table: &mut ActivePageTableInner) -> Result<(), &'static str> {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn)?;
        }
        Ok(())
    }

    /// Copy data to the mapped area
    pub fn copy_data(&self, page_table: &mut ActivePageTableInner, data: &[u8]) {
        let mut copied = 0;
        let total_len = data.len();

        for vpn in self.vpn_range {
            if copied >= total_len {
                break;
            }

            // Get the physical address for this vpn
            let vaddr = vpn.start_addr();
            let paddr = page_table
                .translate(vaddr)
                .expect("failed to translate virtual address");

            // Calculate how much to copy for this page
            let remaining = total_len - copied;
            let to_copy = min(remaining, crate::config::PAGE_SIZE);

            // Copy data to the physical page
            unsafe {
                let dst = paddr.to_vaddr().as_mut_ptr::<u8>();
                let src = data.as_ptr().add(copied);
                core::ptr::copy_nonoverlapping(src, dst, to_copy);
            }

            copied += to_copy;
        }
    }

    /// Clone metadata without cloning the frames
    /// Used for Copy-on-Write (COW) fork
    pub fn clone_metadata(&self) -> Self {
        MappingArea {
            vpn_range: self.vpn_range,
            area_type: self.area_type,
            map_type: self.map_type,
            permission: self.permission,
            frames: BTreeMap::new(), // Don't clone frames
        }
    }
}

/// Huge Page Mapping Implementation
/// 
/// Recommended scenarios for use:
/// * Mapping kernel physical memory regions
/// * Mapping kernel text and rodata segments
/// * Mmap large files
/// * Shared memory regions between processes
/// * Large memory rigions that are infrequently allocated and deallocated
/// 
/// Not recommended scenarios for use:
/// * Use it in User Space
/// * Small memory regions that are frequently allocated and deallocated
/// 
/// TODO: Implement split huge page to small pages (if needed, e.g., for COW)
impl MappingArea {
    /// Map to Huge Pages greedily
    pub fn map_with_huge_page(&mut self, page_table: &mut ActivePageTableInner) -> Result<(), page_table::PagingError> {
        let start_va = self.vpn_range.start().start_addr();
        let end_va = self.vpn_range.end().start_addr();
        let mut current_va = start_va;

        while current_va < end_va {
            let remaining = end_va.as_usize() - current_va.as_usize();
            let current_vpn = Vpn::from_addr_floor(current_va);

            // 贪心算法：优先尝试大页

            // 尝试 1GB 页
            if remaining >= PageSize::Size1G as usize
                && current_va.as_usize() % (PageSize::Size1G as usize) == 0
            {
                let ppn = self.allocate_for_huge_page(
                    current_vpn,
                    262144,  // 1GB = 262144 pages
                )?;

                page_table.map(
                    current_vpn,
                    ppn,
                    PageSize::Size1G,
                    self.permission,
                )?;

                current_va = current_va + PageSize::Size1G as usize;
            }
            // 尝试 2MB 页
            else if remaining >= PageSize::Size2M as usize
                && current_va.as_usize() % (PageSize::Size2M as usize) == 0
            {
                let ppn = self.allocate_for_huge_page(
                    current_vpn,
                    512,  // 2MB = 512 pages
                )?;

                page_table.map(
                    current_vpn,
                    ppn,
                    PageSize::Size2M,
                    self.permission,
                )?;

                current_va = current_va + PageSize::Size2M as usize;
            }
            // 使用 4KB 页
            else {
                let ppn = self.allocate_for_small_page(current_vpn)?;

                page_table.map(
                    current_vpn,
                    ppn,
                    PageSize::Size4K,
                    self.permission,
                )?;

                current_va = current_va + PageSize::Size4K as usize;
            }
        }

        Ok(())
    }

    fn allocate_for_huge_page(&mut self, vpn: Vpn, num_pages: usize) -> Result<Ppn, page_table::PagingError> {
        match self.map_type {
            MapType::Identical => {
                // 恒等映射：PPN = VPN
                Ok(Ppn::from_usize(vpn.as_usize()))
            }
            MapType::Framed => {
                // 使用对齐分配
                let frame_range = crate::mm::frame_allocator::alloc_contig_frames_aligned(
                    num_pages,
                    num_pages,  // 对齐要求 = 页数
                ).ok_or(page_table::PagingError::InvalidAddress)?;

                let ppn = frame_range.start_ppn();

                // 存储到 frames（使用 Contiguous 变体）
                self.frames.insert(vpn, TrackedFrames::Contiguous(frame_range));

                Ok(ppn)
            }
        }
    }

    fn allocate_for_small_page(&mut self, vpn: Vpn) -> Result<Ppn, page_table::PagingError> {
        match self.map_type {
            MapType::Identical => {
                Ok(Ppn::from_usize(vpn.as_usize()))
            }
            MapType::Framed => {
                let frame = crate::mm::frame_allocator::alloc_frame()
                    .ok_or(page_table::PagingError::InvalidAddress)?;
                let ppn = frame.ppn();
                self.frames.insert(vpn, TrackedFrames::Single(frame));
                Ok(ppn)
            }
        }
    }
}
