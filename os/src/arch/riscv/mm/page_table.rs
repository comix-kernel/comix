use super::PageTableEntry;
use crate::mm::address::{ConvertablePaddr, Paddr, PageNum, Ppn, UsizeConvert, Vaddr, Vpn};
use crate::mm::frame_allocator::{FrameTracker, alloc_frame};
use crate::mm::page_table::{
    PageSize, PageTableEntry as PageTableEntryTrait, PageTableInner as PageTableInnerTrait,
    PagingError, PagingResult, UniversalPTEFlag,
};
use alloc::vec::Vec;

#[derive(Debug)]
pub struct PageTableInner {
    root: Ppn,
    // only store middle-level frames here
    frames: Vec<FrameTracker>,
    is_user: bool,
}

impl PageTableInnerTrait<PageTableEntry> for PageTableInner {
    const LEVELS: usize = 3;
    const MAX_VA_BITS: usize = 39;
    const MAX_PA_BITS: usize = 56;

    fn tlb_flush(vpn: Vpn) {
        let vaddr = vpn.start_addr();
        unsafe {
            core::arch::asm!(
                "sfence.vma {0} {1}",
                in(reg) vaddr.as_usize(),
                in(reg) 0usize
            )
        }
    }

    fn tlb_flush_all() {
        unsafe { core::arch::asm!("sfence.vma") }
    }

    fn is_user_table(&self) -> bool {
        self.is_user
    }

    fn activate(ppn: Ppn) {
        let satp_value = ppn_to_satp(ppn);
        unsafe {
            core::arch::asm!(
                "csrw satp, {0}",
                "sfence.vma",
                in(reg) satp_value
            )
        }
    }

    fn activating_table_ppn() -> Ppn {
        let satp_value: usize;
        unsafe {
            core::arch::asm!("csrr {0}, satp", out(reg) satp_value);
        }
        let ppn = satp_value & ((1usize << 44) - 1); // lower 44 bits for PPN in SV39
        Ppn::from_usize(ppn)
    }

    fn new() -> Self {
        let frame = alloc_frame().unwrap();
        Self {
            root: frame.ppn(),
            frames: alloc::vec![frame],
            is_user: true,
        }
    }
    fn from_ppn(ppn: Ppn) -> Self {
        Self {
            root: ppn,
            frames: Vec::new(),
            is_user: true,
        }
    }
    fn new_as_kernel_table() -> Self {
        let frame = alloc_frame().unwrap();
        Self {
            root: frame.ppn(),
            frames: alloc::vec![frame],
            is_user: false,
        }
    }

    fn root_ppn(&self) -> Ppn {
        self.root
    }

    fn get_entry(&self, vpn: Vpn, level: usize) -> Option<(PageTableEntry, PageSize)> {
        if level >= Self::LEVELS {
            return None;
        }

        let mut ppn = self.root;
        let vpn_value = vpn.as_usize();

        // Walk through page table levels from root to the target level
        for current_level in (level..Self::LEVELS).rev() {
            let idx = (vpn_value >> (9 * current_level)) & 0x1ff;
            let pte_array = unsafe {
                core::slice::from_raw_parts(
                    ppn.start_addr().to_vaddr().as_usize() as *const PageTableEntry,
                    512,
                )
            };
            let pte = &pte_array[idx];

            if !pte.is_valid() {
                return None;
            }

            if current_level == level {
                // TODO(暂时注释): 当前仅支持4K页
                // let page_size = match level {
                //     2 => PageSize::Size1G,
                //     1 => PageSize::Size2M,
                //     0 => PageSize::Size4K,
                //     _ => unreachable!(),
                // };
                let page_size = PageSize::Size4K; // 仅支持4K页
                return Some((*pte, page_size));
            }

            ppn = pte.ppn();
        }

        None
    }

    fn translate(&self, vaddr: Vaddr) -> Option<Paddr> {
        let vpn = Vpn::from_addr_ceil(vaddr);
        let offset = vaddr.as_usize() & 0xfff; // Lower 12 bits for page offset

        // TODO(暂时注释): 当前仅支持4K页，大页translation逻辑已禁用
        match self.walk(vpn) {
            Ok((ppn, page_size, _flags)) => {
                let paddr_base = match page_size {
                    PageSize::Size4K => ppn.start_addr().as_usize(),
                    // TODO(暂时注释): 大页偏移计算
                    // PageSize::Size2M => {
                    //     // For 2M pages, preserve the lower 21 bits from vaddr
                    //     let offset_2m = vaddr.as_usize() & 0x1f_ffff;
                    //     ppn.start_addr().as_usize() + offset_2m - offset
                    // }
                    // PageSize::Size1G => {
                    //     // For 1G pages, preserve the lower 30 bits from vaddr
                    //     let offset_1g = vaddr.as_usize() & 0x3fff_ffff;
                    //     ppn.start_addr().as_usize() + offset_1g - offset
                    // }
                    _ => ppn.start_addr().as_usize(), // 默认按4K处理
                };
                Some(Paddr::from_usize(paddr_base + offset))
            }
            Err(_) => None,
        }
    }

    fn map(
        &mut self,
        vpn: Vpn,
        ppn: Ppn,
        _page_size: PageSize,
        flags: UniversalPTEFlag,
    ) -> PagingResult<()> {
        // Validate flags: leaf pages must have at least one of R/W/X set
        if !flags.intersects(
            UniversalPTEFlag::READABLE | UniversalPTEFlag::WRITEABLE | UniversalPTEFlag::EXECUTABLE,
        ) {
            return Err(PagingError::InvalidFlags);
        }

        // TODO(暂时注释): 当前仅支持4K页，强制使用level 0
        // Determine the target level based on page size
        // let target_level = match page_size {
        //     PageSize::Size1G => 2,
        //     PageSize::Size2M => 1,
        //     PageSize::Size4K => 0,
        // };
        let target_level = 0; // 仅支持4K页

        let mut current_ppn = self.root;
        let vpn_value = vpn.as_usize();

        // Walk through page table levels from root to target level
        for level in (target_level..Self::LEVELS).rev() {
            let idx = (vpn_value >> (9 * level)) & 0x1ff;
            let pte_array = unsafe {
                core::slice::from_raw_parts_mut(
                    current_ppn.start_addr().to_vaddr().as_usize() as *mut PageTableEntry,
                    512,
                )
            };
            let pte = &mut pte_array[idx];

            if level == target_level {
                // We've reached the target level, create leaf entry
                if pte.is_valid() {
                    return Err(PagingError::AlreadyMapped);
                }
                *pte = PageTableEntry::new_leaf(ppn, flags | UniversalPTEFlag::VALID);
                return Ok(());
            } else {
                // Intermediate level - need to continue walking
                if !pte.is_valid() {
                    // Allocate a new page table for this level
                    let new_frame = alloc_frame().ok_or(PagingError::FrameAllocFailed)?;
                    let new_ppn = new_frame.ppn();

                    // Clear the new page table
                    let new_table = unsafe {
                        core::slice::from_raw_parts_mut(
                            new_ppn.start_addr().to_vaddr().as_usize() as *mut PageTableEntry,
                            512,
                        )
                    };
                    for entry in new_table.iter_mut() {
                        *entry = PageTableEntry::empty();
                    }

                    *pte = PageTableEntry::new_table(new_ppn);
                    self.frames.push(new_frame);
                } else if pte.is_huge() {
                    // There's already a huge page mapping here
                    return Err(PagingError::HugePageConflict);
                }

                current_ppn = pte.ppn();
            }
        }

        Err(PagingError::InvalidAddress)
    }

    fn unmap(&mut self, vpn: Vpn) -> PagingResult<()> {
        let mut current_ppn = self.root;
        let vpn_value = vpn.as_usize();

        // Walk through page table to find the leaf entry
        for level in (0..Self::LEVELS).rev() {
            let idx = (vpn_value >> (9 * level)) & 0x1ff;
            let pte_array = unsafe {
                core::slice::from_raw_parts_mut(
                    current_ppn.start_addr().to_vaddr().as_usize() as *mut PageTableEntry,
                    512,
                )
            };
            let pte = &mut pte_array[idx];

            if !pte.is_valid() {
                return Err(PagingError::NotMapped);
            }

            // Check if this is a leaf entry (has R/W/X permissions or is level 0)
            if pte.is_huge() || level == 0 {
                Self::tlb_flush(vpn);
                return Ok(());
            }

            current_ppn = pte.ppn();
        }

        Err(PagingError::NotMapped)
    }

    fn mvmap(
        &mut self,
        vpn: Vpn,
        target_ppn: Ppn,
        page_size: PageSize,
        flags: UniversalPTEFlag,
    ) -> PagingResult<()> {
        // First unmap the old mapping
        self.unmap(vpn)?;
        // Then map to the new physical page
        self.map(vpn, target_ppn, page_size, flags)
    }

    fn update_flags(&mut self, vpn: Vpn, flags: UniversalPTEFlag) -> PagingResult<()> {
        let mut current_ppn = self.root;
        let vpn_value = vpn.as_usize();

        // Walk through page table to find the leaf entry
        for level in (0..Self::LEVELS).rev() {
            let idx = (vpn_value >> (9 * level)) & 0x1ff;
            let pte_array = unsafe {
                core::slice::from_raw_parts_mut(
                    current_ppn.start_addr().to_vaddr().as_usize() as *mut PageTableEntry,
                    512,
                )
            };
            let pte = &mut pte_array[idx];

            if !pte.is_valid() {
                return Err(PagingError::NotMapped);
            }

            // Check if this is a leaf entry (has R/W/X permissions or is level 0)
            if pte.is_huge() || level == 0 {
                pte.set_flags(flags | UniversalPTEFlag::VALID);
                Self::tlb_flush(vpn);
                return Ok(());
            }

            current_ppn = pte.ppn();
        }

        Err(PagingError::NotMapped)
    }

    fn walk(&self, vpn: Vpn) -> PagingResult<(Ppn, PageSize, UniversalPTEFlag)> {
        let mut ppn = self.root;
        let vpn_value = vpn.as_usize();

        // SV39: VPN[2] = bits[38:30], VPN[1] = bits[29:21], VPN[0] = bits[20:12]
        for level in (0..Self::LEVELS).rev() {
            let idx = (vpn_value >> (9 * level)) & 0x1ff;
            let pte_array = unsafe {
                core::slice::from_raw_parts(
                    ppn.start_addr().to_vaddr().as_usize() as *const PageTableEntry,
                    512,
                )
            };
            let pte = &pte_array[idx];

            if !pte.is_valid() {
                return Err(PagingError::NotMapped);
            }

            if pte.is_huge() || level == 0 {
                // TODO(暂时注释): 当前仅支持4K页
                // let page_size = match level {
                //     2 => PageSize::Size1G,
                //     1 => PageSize::Size2M,
                //     0 => PageSize::Size4K,
                //     _ => unreachable!(),
                // };
                let page_size = PageSize::Size4K; // 仅支持4K页
                return Ok((pte.ppn(), page_size, pte.flags()));
            }

            ppn = pte.ppn();
        }

        Err(PagingError::NotMapped)
    }
}

fn ppn_to_satp(ppn: Ppn) -> usize {
    ppn.as_usize() | (8usize << 60) // MODE=8 for SV39
}

#[cfg(test)]
mod page_table_tests {
    use super::*;
    use crate::mm::page_table::PageTableInner as PageTableInnerTrait;
    use crate::{test_case, kassert};

    // 1. Page table creation
    test_case!(test_pt_create, {
        let pt = PageTableInner::new();
        // Root PPN should be valid
        kassert!(pt.root_ppn().as_usize() > 0);
        kassert!(pt.is_user_table());
    });

    // 2. Map and translate
    test_case!(test_pt_map_translate, {
        let mut pt = PageTableInner::new();
        let vpn = Vpn::from_usize(0x1000);
        let ppn = Ppn::from_usize(0x80000);
        let vaddr = Vaddr::from_usize(0x1000_0000); // vpn 0x1000 << 12

        // Map vpn -> ppn
        let result = pt.map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::kernel_rw());
        kassert!(result.is_ok());

        // Translate back
        let translated = pt.translate(vaddr);
        kassert!(translated.is_some());
        let paddr = translated.unwrap();
        kassert!(paddr.as_usize() >> 12 == ppn.as_usize());
    });

    // 3. Unmap
    test_case!(test_pt_unmap, {
        let mut pt = PageTableInner::new();
        let vpn = Vpn::from_usize(0x1000);
        let ppn = Ppn::from_usize(0x80000);

        // Map first
        pt.map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::kernel_rw()).unwrap();

        // Unmap
        let result = pt.unmap(vpn);
        kassert!(result.is_ok());

        // Should not be mapped anymore
        let vaddr = Vaddr::from_usize(0x1000_0000);
        let translated = pt.translate(vaddr);
        kassert!(translated.is_none());
    });

    // 4. Error: already mapped
    test_case!(test_pt_error_already_mapped, {
        let mut pt = PageTableInner::new();
        let vpn = Vpn::from_usize(0x1000);

        // First mapping succeeds
        let result1 = pt.map(vpn, Ppn::from_usize(0x80000), PageSize::Size4K, UniversalPTEFlag::kernel_rw());
        kassert!(result1.is_ok());

        // Second mapping should fail
        let result2 = pt.map(vpn, Ppn::from_usize(0x80001), PageSize::Size4K, UniversalPTEFlag::kernel_rw());
        kassert!(result2.is_err());
    });

    // 5. Walk page table
    test_case!(test_pt_walk, {
        let mut pt = PageTableInner::new();
        let vpn = Vpn::from_usize(0x1000);
        let ppn = Ppn::from_usize(0x80000);
        let flags = UniversalPTEFlag::kernel_rw();

        // Map first
        pt.map(vpn, ppn, PageSize::Size4K, flags).unwrap();

        // Walk to get mapping info
        let walk_result = pt.walk(vpn);
        kassert!(walk_result.is_ok());

        let (mapped_ppn, _, mapped_flags) = walk_result.unwrap();
        kassert!(mapped_ppn == ppn);
        kassert!(mapped_flags.bits() == flags.bits());
    });

    // 6. Update flags
    test_case!(test_pt_update_flags, {
        let mut pt = PageTableInner::new();
        let vpn = Vpn::from_usize(0x1000);
        let ppn = Ppn::from_usize(0x80000);

        // Map with kernel_rw
        pt.map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::kernel_rw()).unwrap();

        // Update to kernel read-only
        let new_flags = UniversalPTEFlag::kernel_r();
        let result = pt.update_flags(vpn, new_flags);
        kassert!(result.is_ok());

        // Verify flags changed
        let (_, _, flags) = pt.walk(vpn).unwrap();
        kassert!(flags.bits() == new_flags.bits());
    });

    // 7. Multiple mappings
    test_case!(test_pt_multiple_mappings, {
        let mut pt = PageTableInner::new();

        // Map multiple VPNs
        for i in 0..10 {
            let vpn = Vpn::from_usize(0x1000 + i);
            let ppn = Ppn::from_usize(0x80000 + i);
            let result = pt.map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::kernel_rw());
            kassert!(result.is_ok());
        }

        // Verify all mappings
        for i in 0..10 {
            let vpn = Vpn::from_usize(0x1000 + i);
            let expected_ppn = Ppn::from_usize(0x80000 + i);
            let (mapped_ppn, _, _) = pt.walk(vpn).unwrap();
            kassert!(mapped_ppn == expected_ppn);
        }
    });
}
