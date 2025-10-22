use alloc::collections::btree_map::BTreeMap;
use core::cmp::min;

use crate::mm::address::{Vpn, VpnRange, UsizeConvert, ConvertablePaddr, PageNum};
use crate::mm::frame_allocator::{TrackedFrames, alloc_frame};
use crate::mm::page_table::{ActivePageTableInner, PageTableInner, UniversalPTEFlag, PageSize};

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

    /// Permission flags for this mapping area (use UniversalPTEFlag for perfermance)
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
    pub fn map_one(&mut self, page_table: &mut ActivePageTableInner, vpn: Vpn) {
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
    }

    /// Map all pages in this mapping area
    pub fn map(&mut self, page_table: &mut ActivePageTableInner) {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn);
        }
    }

    /// Unmap a single virtual page
    pub fn unmap_one(&mut self, page_table: &mut ActivePageTableInner, vpn: Vpn) {
        page_table.unmap(vpn).expect("failed to unmap page");

        // For framed mapping, remove the frame tracker
        if self.map_type == MapType::Framed {
            self.frames.remove(&vpn);
        }
    }

    /// Unmap all pages in this mapping area
    pub fn unmap(&mut self, page_table: &mut ActivePageTableInner) {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn);
        }
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
