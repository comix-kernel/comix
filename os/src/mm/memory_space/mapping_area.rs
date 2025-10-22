use alloc::collections::btree_map::BTreeMap;

use crate::mm::address::{Vpn, VpnRange};
use crate::mm::frame_allocator::TrackedFrames;
use crate::mm::page_table::{ActivePageTableInner, PageTableInner, UniversalPTEFlag};

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

    // TODO: implement map_one
    pub fn map_one(&mut self, page_table: &mut ActivePageTableInner, vpn: Vpn) {
        unimplemented!("map_one in MappingArea");
    }

    // TODO: implement map
    pub fn map(&mut self, page_table: &mut ActivePageTableInner) {
        unimplemented!("map in MappingArea");
    }

    // TODO: implement unmap_one
    pub fn unmap_one(&mut self, page_table: &mut ActivePageTableInner, vpn: Vpn) {
        unimplemented!("unmap_one in MappingArea");
    }

    // TODO: implement unmap
    pub fn unmap(&mut self, page_table: &mut ActivePageTableInner) {
        unimplemented!("unmap in MappingArea");
    }

    // TODO: implement copy_data
    pub fn copy_data(&self, page_table: &mut ActivePageTableInner, data: &[u8]) {
        unimplemented!("copy_data in MappingArea");
    }

    // TODO: implement clone_metadata
    pub fn clone_metadata(&self) -> Self {
        unimplemented!("clone_metadata in MappingArea");
    }
}
