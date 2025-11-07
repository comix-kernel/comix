use core::cmp::Ordering;

use crate::arch::mm::{paddr_to_vaddr, vaddr_to_paddr};
use crate::config::{
    MAX_USER_HEAP_SIZE, MEMORY_END, TRAMPOLINE, TRAP_CONTEXT, USER_STACK_SIZE, USER_STACK_TOP,
};
use crate::mm::address::{Paddr, PageNum, Ppn, UsizeConvert, Vaddr, Vpn, VpnRange};
use crate::mm::memory_space::mapping_area::{AreaType, MapType, MappingArea};
use crate::mm::page_table::{
    ActivePageTableInner, PageSize, PageTableInner, PagingError, UniversalPTEFlag,
};
use crate::sync::spin_lock::SpinLock;
use alloc::vec::Vec;
use lazy_static::lazy_static;

// Kernel linker symbols
unsafe extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss();
    fn ebss();
    fn ekernel();
    fn strampoline();
}

lazy_static! {
    /// Global kernel memory space (protected by SpinLock)
    static ref KERNEL_SPACE: SpinLock<MemorySpace> = {
        SpinLock::new(MemorySpace::new_kernel())
    };
}

/// Returns the kernel page table token
pub fn kernel_token() -> usize {
    (KERNEL_SPACE.lock().page_table.root_ppn().as_usize() << 44) | (8 << 60)
}

/// Returns the kernel root PPN
pub fn kernel_root_ppn() -> Ppn {
    KERNEL_SPACE.lock().root_ppn()
}

/// Executes a closure with exclusive access to kernel space
pub fn with_kernel_space<F, R>(f: F) -> R
where
    F: FnOnce(&mut MemorySpace) -> R,
{
    let mut guard = KERNEL_SPACE.lock();
    f(&mut guard)
}

/// Memory space structure representing an address space
#[derive(Debug)]
pub struct MemorySpace {
    /// Page table associated with this memory space
    page_table: ActivePageTableInner,

    /// List of mapping areas in this memory space
    areas: Vec<MappingArea>,

    /// Heap top for brk system call (user space only)
    heap_top: Option<Vpn>,
}

impl MemorySpace {
    /// Creates a new empty memory space
    pub fn new() -> Self {
        MemorySpace {
            page_table: ActivePageTableInner::new(),
            areas: Vec::new(),
            heap_top: None,
        }
    }

    /// Returns a reference to the page table
    pub fn page_table(&self) -> &ActivePageTableInner {
        &self.page_table
    }

    /// Returns a mutable reference to the page table
    pub fn page_table_mut(&mut self) -> &mut ActivePageTableInner {
        &mut self.page_table
    }

    /// Returns the root page table PPN
    pub fn root_ppn(&self) -> Ppn {
        self.page_table.root_ppn()
    }

    /// Maps the trampoline page to both kernel and user space
    fn map_trampoline(&mut self) -> Result<(), PagingError> {
        let trampoline_vpn = Vpn::from_addr_floor(Vaddr::from_usize(TRAMPOLINE));

        // High-half kernel: strampoline is a virtual address, needs conversion to physical
        let strampoline_paddr = unsafe { vaddr_to_paddr(strampoline as usize) };
        let trampoline_ppn = Ppn::from_addr_floor(Paddr::from_usize(strampoline_paddr));

        self.page_table.map(
            trampoline_vpn,
            trampoline_ppn,
            PageSize::Size4K,
            UniversalPTEFlag::kernel_r() | UniversalPTEFlag::EXECUTABLE,
        )?;

        Ok(())
    }

    /// Maps the trampoline page to user space (with user access permission)
    fn map_trampoline_user(&mut self) -> Result<(), PagingError> {
        let trampoline_vpn = Vpn::from_addr_floor(Vaddr::from_usize(TRAMPOLINE));

        // High-half kernel: strampoline is a virtual address, needs conversion to physical
        let strampoline_paddr = unsafe { vaddr_to_paddr(strampoline as usize) };
        let trampoline_ppn = Ppn::from_addr_floor(Paddr::from_usize(strampoline_paddr));

        self.page_table.map(
            trampoline_vpn,
            trampoline_ppn,
            PageSize::Size4K,
            UniversalPTEFlag::USER_ACCESSIBLE
                | UniversalPTEFlag::READABLE
                | UniversalPTEFlag::EXECUTABLE,
        )?;

        Ok(())
    }

    /// Maps kernel space (shared across all address spaces)
    ///
    /// This method implements the core logic of Scheme 2 (Shared Page Table):
    /// Every user process's page table contains both user space mappings (private)
    /// and kernel space mappings (shared). This design enables zero-overhead
    /// user/kernel mode switching without changing the `satp` register.
    ///
    /// # Arguments
    /// - `include_trampoline`: Whether to include trampoline mapping with kernel permissions (U=0)
    ///
    /// # Mapping Contents
    /// All mappings use Direct mode (VA = PA + VADDR_START) and have U=0 flag:
    /// - Trampoline page (optional): R+X, Direct mapping
    /// - Kernel .text segment: R+X, Direct mapping
    /// - Kernel .rodata segment: R, Direct mapping
    /// - Kernel .data segment: R+W, Direct mapping
    /// - Kernel .bss.stack segment: R+W, Direct mapping
    /// - Kernel .bss segment: R+W, Direct mapping
    /// - Kernel heap: R+W, Direct mapping
    /// - Physical memory: R+W, Direct mapping
    ///
    /// # Security
    /// All kernel mappings have the U (User Accessible) flag set to 0, ensuring
    /// that user mode cannot access kernel memory despite it being present in the
    /// page table. This is enforced by hardware.
    ///
    /// # Architecture
    /// Current implementation targets RISC-V SV39. Other architectures need to
    /// adjust address ranges accordingly.
    fn map_kernel_space(&mut self, include_trampoline: bool) -> Result<(), PagingError> {
        unsafe extern "C" {
            fn stext();
            fn etext();
            fn srodata();
            fn erodata();
            fn sdata();
            fn edata();
            fn sbss();
            fn ebss();
            fn ekernel();
            fn strampoline();
        }

        // 0. Map trampoline (optional, with kernel permissions)
        if include_trampoline {
            self.map_trampoline()?;
        }

        // 1. Map kernel .text segment (read + execute)
        Self::map_kernel_section(
            self,
            stext as usize,
            etext as usize,
            AreaType::KernelText,
            UniversalPTEFlag::kernel_rx(),
        )?;

        // 2. Map kernel .rodata segment (read-only)
        Self::map_kernel_section(
            self,
            srodata as usize,
            erodata as usize,
            AreaType::KernelRodata,
            UniversalPTEFlag::kernel_r(),
        )?;

        // 3. Map kernel .data segment (read-write)
        Self::map_kernel_section(
            self,
            sdata as usize,
            edata as usize,
            AreaType::KernelData,
            UniversalPTEFlag::kernel_rw(),
        )?;

        // 4a. Map kernel boot stack (.bss.stack section)
        Self::map_kernel_section(
            self,
            edata as usize, // .bss.stack starts at edata
            sbss as usize,  // .bss.stack ends at sbss
            AreaType::KernelStack,
            UniversalPTEFlag::kernel_rw(),
        )?;

        // 4b. Map kernel .bss segment
        Self::map_kernel_section(
            self,
            sbss as usize,
            ebss as usize,
            AreaType::KernelBss,
            UniversalPTEFlag::kernel_rw(),
        )?;

        // 4c. Map kernel heap
        Self::map_kernel_section(
            self,
            ebss as usize,    // sheap
            ekernel as usize, // eheap
            AreaType::KernelHeap,
            UniversalPTEFlag::kernel_rw(),
        )?;

        // 5. Map physical memory (direct mapping from ekernel to MEMORY_END)
        let ekernel_paddr = unsafe { vaddr_to_paddr(ekernel as usize) };
        let phys_mem_start_vaddr = paddr_to_vaddr(ekernel_paddr);
        let phys_mem_end_vaddr = paddr_to_vaddr(MEMORY_END);

        let phys_mem_start = Vpn::from_addr_ceil(Vaddr::from_usize(phys_mem_start_vaddr));
        let phys_mem_end = Vpn::from_addr_floor(Vaddr::from_usize(phys_mem_end_vaddr));
        let mut phys_mem_area = MappingArea::new(
            VpnRange::new(phys_mem_start, phys_mem_end),
            AreaType::KernelHeap,
            MapType::Direct,
            UniversalPTEFlag::kernel_rw(),
        );

        phys_mem_area.map(&mut self.page_table)?;
        self.areas.push(phys_mem_area);

        Ok(())
    }

    /// Inserts a new mapping area with overlap detection
    ///
    /// # Errors
    /// Returns error if the area overlaps with existing areas
    pub fn insert_area(&mut self, mut area: MappingArea) -> Result<(), PagingError> {
        // 1. Check for overlaps
        for existing in &self.areas {
            if existing.vpn_range().overlaps(&area.vpn_range()) {
                return Err(PagingError::AlreadyMapped);
            }
        }

        // 2. Map to page table (if fails, area will be dropped automatically)
        area.map(&mut self.page_table)?;

        // 3. Append to areas list
        self.areas.push(area);

        Ok(())
    }

    /// Inserts a framed area with optional data copying
    pub fn insert_framed_area(
        &mut self,
        vpn_range: VpnRange,
        area_type: AreaType,
        flags: UniversalPTEFlag,
        data: Option<&[u8]>,
    ) -> Result<(), PagingError> {
        let mut area = MappingArea::new(vpn_range, area_type, MapType::Framed, flags);

        // Map pages
        area.map(&mut self.page_table)?;

        // Copy data if provided
        if let Some(data) = data {
            area.copy_data(&mut self.page_table, data);
        }

        // Check overlap and insert
        self.insert_area(area)?;

        Ok(())
    }

    /// Finds the area containing the given VPN
    pub fn find_area(&self, vpn: Vpn) -> Option<&MappingArea> {
        self.areas
            .iter()
            .find(|area| area.vpn_range().contains(vpn))
    }

    /// Finds the area containing the given VPN (mutable)
    pub fn find_area_mut(&mut self, vpn: Vpn) -> Option<&mut MappingArea> {
        self.areas
            .iter_mut()
            .find(|area| area.vpn_range().contains(vpn))
    }

    /// Removes and unmaps an area by VPN
    pub fn remove_area(&mut self, vpn: Vpn) -> Result<(), PagingError> {
        if let Some(pos) = self.areas.iter().position(|a| a.vpn_range().contains(vpn)) {
            let mut area = self.areas.remove(pos);
            area.unmap(&mut self.page_table)?;
            Ok(())
        } else {
            Err(PagingError::NotMapped)
        }
    }

    /// Creates the kernel memory space
    ///
    /// This creates a complete kernel address space including trampoline,
    /// kernel segments (.text, .rodata, .data, .bss, heap), and direct-mapped
    /// physical memory. Used by kernel threads and during system initialization.
    pub fn new_kernel() -> Self {
        let mut space = MemorySpace::new();

        // Map all kernel space (including trampoline with kernel permissions)
        space
            .map_kernel_space(true)
            .expect("Failed to map kernel space");

        space
    }

    /// Helper: Maps a kernel section
    fn map_kernel_section(
        space: &mut MemorySpace,
        start: usize,
        end: usize,
        area_type: AreaType,
        flags: UniversalPTEFlag,
    ) -> Result<(), PagingError> {
        let vpn_start = Vpn::from_addr_floor(Vaddr::from_usize(start));
        let vpn_end = Vpn::from_addr_ceil(Vaddr::from_usize(end));

        let mut area = MappingArea::new(
            VpnRange::new(vpn_start, vpn_end),
            area_type,
            MapType::Direct,
            flags,
        );

        area.map(&mut space.page_table)?;
        space.areas.push(area);
        Ok(())
    }

    /// Helper: Maps an MMIO region
    fn map_mmio_region(
        space: &mut MemorySpace,
        addr: usize,
        size: usize,
    ) -> Result<(), PagingError> {
        let vpn_start = Vpn::from_addr_floor(Vaddr::from_usize(addr));
        let vpn_end = Vpn::from_addr_ceil(Vaddr::from_usize(addr + size));

        let mut area = MappingArea::new(
            VpnRange::new(vpn_start, vpn_end),
            AreaType::KernelMmio,
            MapType::Direct,
            UniversalPTEFlag::kernel_rw(),
        );

        area.map(&mut space.page_table)?;
        space.areas.push(area);
        Ok(())
    }

    /// Creates a user memory space from an ELF file
    ///
    /// This method implements Scheme 2 (Shared Page Table) by creating a page table
    /// that contains both user space mappings (process-private) and kernel space
    /// mappings (shared across all processes).
    ///
    /// The resulting page table enables zero-overhead user/kernel mode switching:
    /// when a user process traps into the kernel, the kernel code is already mapped
    /// and accessible without changing `satp`.
    ///
    /// # Returns
    /// Returns `Ok((space, entry_point, user_stack_top))` on success:
    /// - `space`: The constructed memory space with user + kernel mappings
    /// - `entry_point`: The program entry address (from ELF header)
    /// - `user_stack_top`: The top of the user stack
    ///
    /// # Errors
    /// - ELF parsing failed
    /// - Architecture mismatch (not RISC-V)
    /// - Segment overlaps with reserved areas
    pub fn from_elf(elf_data: &[u8]) -> Result<(Self, usize, usize), PagingError> {
        use xmas_elf::ElfFile;
        use xmas_elf::program::{SegmentData, Type};

        let elf = ElfFile::new(elf_data).map_err(|_| PagingError::InvalidAddress)?;

        // Check architecture
        if elf.header.pt2.machine().as_machine() != xmas_elf::header::Machine::RISC_V {
            return Err(PagingError::InvalidAddress);
        }

        let mut space = MemorySpace::new();

        // ========== Scheme 2: Map kernel space first ==========
        // 0. Map kernel space (all processes share the same kernel mappings)
        //    - Excludes trampoline (will be mapped with U=1 below)
        //    - All kernel pages have U=0, so user mode cannot access them
        space
            .map_kernel_space(false)
            .expect("Failed to map kernel space for user process");
        // ======================================================

        // 1. Map trampoline (user space needs user access permission U=1)
        space
            .map_trampoline_user()
            .expect("Failed to map trampoline in user space");

        let mut max_end_vpn = Vpn::from_usize(0);

        // 2. Parse and map ELF segments
        for ph in elf.program_iter() {
            if ph.get_type() != Ok(Type::Load) {
                continue;
            }

            let start_va = ph.virtual_addr() as usize;
            let end_va = (ph.virtual_addr() + ph.mem_size()) as usize;

            // Check if segment overlaps with stack/trap area
            if start_va >= USER_STACK_TOP - USER_STACK_SIZE {
                return Err(PagingError::InvalidAddress);
            }

            let vpn_range = VpnRange::new(
                Vpn::from_addr_floor(Vaddr::from_usize(start_va)),
                Vpn::from_addr_ceil(Vaddr::from_usize(end_va)),
            );

            max_end_vpn = if max_end_vpn.as_usize() > vpn_range.end().as_usize() {
                max_end_vpn
            } else {
                vpn_range.end()
            };

            // Build permissions
            let mut flags = UniversalPTEFlag::USER_ACCESSIBLE | UniversalPTEFlag::VALID;
            if ph.flags().is_read() {
                flags |= UniversalPTEFlag::READABLE;
            }
            if ph.flags().is_write() {
                flags |= UniversalPTEFlag::WRITEABLE;
            }
            if ph.flags().is_execute() {
                flags |= UniversalPTEFlag::EXECUTABLE;
            }

            // Determine area type
            let area_type = if ph.flags().is_execute() {
                AreaType::UserText
            } else if ph.flags().is_write() {
                AreaType::UserData
            } else {
                AreaType::UserRodata
            };

            // Get segment data
            let data = match ph.get_data(&elf) {
                Ok(SegmentData::Undefined(data)) => Some(data),
                _ => None,
            };

            // Insert area (will check overlap internally)
            space.insert_framed_area(vpn_range, area_type, flags, data)?;
        }

        // 3. Initialize heap (starts at ELF end, page-aligned)
        space.heap_top = Some(max_end_vpn);

        // 4. Map user stack (with guard pages)
        let user_stack_bottom =
            Vpn::from_addr_floor(Vaddr::from_usize(USER_STACK_TOP - USER_STACK_SIZE));
        let user_stack_top = Vpn::from_addr_ceil(Vaddr::from_usize(USER_STACK_TOP));

        space.insert_framed_area(
            VpnRange::new(user_stack_bottom, user_stack_top),
            AreaType::UserStack,
            UniversalPTEFlag::user_rw(),
            None,
        )?;

        // 5. Map trap context page
        let trap_cx_vpn = Vpn::from_addr_floor(Vaddr::from_usize(TRAP_CONTEXT));
        space.insert_framed_area(
            VpnRange::from_start_len(trap_cx_vpn, 1),
            AreaType::UserData,
            UniversalPTEFlag::user_rw(),
            None,
        )?;

        let entry_point = elf.header.pt2.entry_point() as usize;

        Ok((space, entry_point, USER_STACK_TOP))
    }

    /// Extends or shrinks the heap area (brk system call)
    ///
    /// # Errors
    /// - Heap not initialized
    /// - New brk would exceed MAX_USER_HEAP_SIZE
    /// - New brk would overlap with existing areas
    pub fn brk(&mut self, new_brk: usize) -> Result<usize, PagingError> {
        let heap_bottom = self.heap_top.ok_or(PagingError::InvalidAddress)?;
        let new_end_vpn = Vpn::from_addr_ceil(Vaddr::from_usize(new_brk));

        // Boundary checks
        if new_brk < heap_bottom.start_addr().as_usize() {
            return Err(PagingError::InvalidAddress);
        }

        let heap_size = new_brk - heap_bottom.start_addr().as_usize();
        if heap_size > MAX_USER_HEAP_SIZE {
            return Err(PagingError::InvalidAddress);
        }

        // Check if overlaps with stack
        if new_brk >= USER_STACK_TOP - USER_STACK_SIZE {
            return Err(PagingError::InvalidAddress);
        }

        // Find or create heap area
        let heap_area = self
            .areas
            .iter_mut()
            .find(|a| a.area_type() == AreaType::UserHeap);

        if let Some(area) = heap_area {
            // Existing heap area, adjust size
            let old_end = area.vpn_range().end();

            match new_end_vpn.cmp(&old_end) {
                Ordering::Greater => {
                    // Extend
                    let count = new_end_vpn.as_usize() - old_end.as_usize();
                    if count != 0 {
                        area.extend(&mut self.page_table, count)?;
                    }
                }
                Ordering::Less => {
                    // Shrink
                    let count = old_end.as_usize() - new_end_vpn.as_usize();
                    if count != 0 {
                        area.shrink(&mut self.page_table, count)?;
                    }
                }
                Ordering::Equal => { /* no-op */ }
            }
        } else {
            // First time allocating heap, create new area
            if new_end_vpn > heap_bottom {
                self.insert_framed_area(
                    VpnRange::new(heap_bottom, new_end_vpn),
                    AreaType::UserHeap,
                    UniversalPTEFlag::user_rw(),
                    None,
                )?;
            }
        }

        Ok(new_brk)
    }

    /// Maps an anonymous region (simplified mmap)
    ///
    /// # Arguments
    /// - `hint`: Suggested start address (0 = kernel chooses)
    /// - `len`: Length in bytes
    /// - `prot`: Protection flags (PROT_READ | PROT_WRITE | PROT_EXEC)
    pub fn mmap(&mut self, hint: usize, len: usize, prot: usize) -> Result<usize, PagingError> {
        if len == 0 {
            return Err(PagingError::InvalidAddress);
        }

        // Determine start address
        let start = if hint == 0 {
            // Kernel chooses address: after heap top
            let heap_end = self
                .heap_top
                .ok_or(PagingError::InvalidAddress)?
                .start_addr()
                .as_usize();

            // Find actual heap end
            self.areas
                .iter()
                .filter(|a| a.area_type() == AreaType::UserHeap)
                .map(|a| a.vpn_range().end().start_addr().as_usize())
                .max()
                .unwrap_or(heap_end)
        } else {
            // User specified address, check if available
            if hint >= USER_STACK_TOP - USER_STACK_SIZE {
                return Err(PagingError::InvalidAddress);
            }
            hint
        };

        let vpn_range = VpnRange::new(
            Vpn::from_addr_floor(Vaddr::from_usize(start)),
            Vpn::from_addr_ceil(Vaddr::from_usize(start + len)),
        );

        // Check overlap
        for area in &self.areas {
            if area.vpn_range().overlaps(&vpn_range) {
                return Err(PagingError::AlreadyMapped);
            }
        }

        // Convert permissions
        let mut flags = UniversalPTEFlag::USER_ACCESSIBLE | UniversalPTEFlag::VALID;
        if prot & 0x1 != 0 {
            flags |= UniversalPTEFlag::READABLE;
        }
        if prot & 0x2 != 0 {
            flags |= UniversalPTEFlag::WRITEABLE;
        }
        if prot & 0x4 != 0 {
            flags |= UniversalPTEFlag::EXECUTABLE;
        }

        self.insert_framed_area(vpn_range, AreaType::UserHeap, flags, None)?;

        Ok(start)
    }

    /// Unmaps a region (munmap system call)
    pub fn munmap(&mut self, start: usize, _len: usize) -> Result<(), PagingError> {
        let vpn = Vpn::from_addr_floor(Vaddr::from_usize(start));
        self.remove_area(vpn)
    }

    /// Clones the memory space (for fork system call)
    ///
    /// # Note
    /// - Direct mappings are shared (no copy)
    /// - Framed mappings are deep copied
    pub fn clone_for_fork(&self) -> Result<Self, PagingError> {
        let mut new_space = MemorySpace::new();
        new_space.heap_top = self.heap_top;

        for area in &self.areas {
            match area.map_type() {
                MapType::Direct => {
                    // Direct mapping: clone metadata and remap to new page table
                    let mut new_area = area.clone_metadata();
                    new_area.map(&mut new_space.page_table)?;
                    new_space.areas.push(new_area);
                }
                MapType::Framed => {
                    // Framed mapping: deep copy data
                    let new_area = area.clone_with_data(&mut new_space.page_table)?;
                    new_space.areas.push(new_area);
                }
            }
        }

        Ok(new_space)
    }
}

#[cfg(test)]
mod memory_space_tests {
    use super::*;
    use crate::mm::address::{Vpn, VpnRange};
    use crate::mm::page_table::UniversalPTEFlag;
    use crate::{kassert, test_case};

    // 1. Create memory space
    test_case!(test_memspace_create, {
        #[allow(unused)]
        let ms = MemorySpace::new();
        // Should have page table initialized
    });

    // 2. Direct mapping
    test_case!(test_direct_mapping, {
        let mut ms = MemorySpace::new();
        let vpn_range = VpnRange::new(Vpn::from_usize(0x80000), Vpn::from_usize(0x80010));

        let area = MappingArea::new(
            vpn_range,
            AreaType::KernelData,
            MapType::Direct,
            UniversalPTEFlag::kernel_rw(),
        );

        ms.insert_area(area).expect("add area failed");
    });

    // 3. Framed mapping
    test_case!(test_framed_mapping, {
        let mut ms = MemorySpace::new();
        let vpn_range = VpnRange::new(Vpn::from_usize(0x1000), Vpn::from_usize(0x1010));

        let area = MappingArea::new(
            vpn_range,
            AreaType::UserData,
            MapType::Framed,
            UniversalPTEFlag::user_rw(),
        );

        ms.insert_area(area).expect("add area failed");
        // Frames auto-allocated for framed mapping
    });

    // 4. Kernel space access
    test_case!(test_kernel_space, {
        use crate::mm::memory_space::memory_space::kernel_token;

        let token = kernel_token();
        kassert!(token > 0); // Valid SATP value
    });
}
