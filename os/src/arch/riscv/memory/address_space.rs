//! RISC-V 进程地址空间 (UserAddressSpace) 实现
//!
//! 包装 `MemorySpace`，提供用户地址空间的标准 trait 接口。

use alloc::vec::Vec;

use crate::config::PAGE_SIZE;
use crate::hal::virtual_memory::{
    PageFrame, PageInfo, PtePermissions, UserAddressSpace, VirtMemoryRegion,
};
use crate::mm::address::{PageNum, Ppn, UsizeConvert, Vaddr, Vpn, VpnRange};
use crate::mm::memory_space::MemorySpace;
use crate::mm::page_table::PageTableInner;
use crate::mm::page_table::{PageSize, UniversalPTEFlag};

/// RISC-V 进程地址空间
///
/// 包装 `MemorySpace` 以提供 `UserAddressSpace` trait 实现。
pub struct Riscv64ProcessAddressSpace {
    inner: MemorySpace,
}

impl Riscv64ProcessAddressSpace {
    pub fn new() -> Self {
        Self {
            inner: MemorySpace::new(),
        }
    }

    pub fn inner(&self) -> &MemorySpace {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut MemorySpace {
        &mut self.inner
    }
}

impl UserAddressSpace for Riscv64ProcessAddressSpace {
    fn new() -> Result<Self, ()> {
        Ok(Self::new())
    }

    fn activate(&self) {
        let root_ppn = self.inner.root_ppn();
        crate::arch::mm::PageTableInner::activate(root_ppn);
    }

    fn deactivate(&self) {
        // Deactivation is handled by switching to another space
    }

    fn map_page(&mut self, page: PageFrame, va: usize, perms: PtePermissions) -> Result<(), ()> {
        let ppn = Ppn::from_usize(page.ppn);
        let vaddr = Vaddr::from_usize(va);
        let vpn = Vpn::from_addr_floor(vaddr);
        let flags = UniversalPTEFlag::from_bits_truncate(perms.bits());
        self.inner
            .page_table_mut()
            .map(vpn, ppn, PageSize::Size4K, flags)
            .map_err(|_| ())
    }

    fn unmap(&mut self, va: usize) -> Result<PageFrame, ()> {
        let vaddr = Vaddr::from_usize(va);
        let vpn = Vpn::from_addr_floor(vaddr);
        // Walk to get ppn before unmapping
        let (ppn, _size, _flags) = self.inner.page_table().walk(vpn).map_err(|_| ())?;
        self.inner.page_table_mut().unmap(vpn).map_err(|_| ())?;
        Ok(PageFrame {
            ppn: ppn.as_usize(),
        })
    }

    fn remap(
        &mut self,
        va: usize,
        new_page: PageFrame,
        perms: PtePermissions,
    ) -> Result<PageFrame, ()> {
        let old = self.unmap(va)?;
        self.map_page(new_page, va, perms)?;
        Ok(old)
    }

    fn protect_range(
        &mut self,
        region: VirtMemoryRegion,
        perms: PtePermissions,
    ) -> Result<(), ()> {
        let start_vpn = Vpn::from_addr_floor(Vaddr::from_usize(region.start_va));
        let end_vpn = Vpn::from_addr_ceil(Vaddr::from_usize(region.start_va + region.len));
        let flags = UniversalPTEFlag::from_bits_truncate(perms.bits());

        for vpn in VpnRange::new(start_vpn, end_vpn) {
            self.inner
                .page_table_mut()
                .update_flags(vpn, flags)
                .map_err(|_| ())?;
        }
        Ok(())
    }

    fn unmap_range(&mut self, region: VirtMemoryRegion) -> Result<Vec<PageFrame>, ()> {
        let start_vpn = Vpn::from_addr_floor(Vaddr::from_usize(region.start_va));
        let end_vpn = Vpn::from_addr_ceil(Vaddr::from_usize(region.start_va + region.len));
        let range = VpnRange::new(start_vpn, end_vpn);

        let mut frames = Vec::new();
        for vpn in range {
            if let Ok((ppn, _size, _flags)) = self.inner.page_table().walk(vpn) {
                self.inner.page_table_mut().unmap(vpn).map_err(|_| ())?;
                frames.push(PageFrame {
                    ppn: ppn.as_usize(),
                });
            }
        }
        Ok(frames)
    }

    fn translate(&self, va: usize) -> Option<PageInfo> {
        let vaddr = Vaddr::from_usize(va);
        let vpn = Vpn::from_addr_floor(vaddr);
        self.inner.page_table().walk(vpn).ok().map(
            |(ppn, _size, flags)| PageInfo {
                ppn: ppn.as_usize(),
                perms: PtePermissions::from_bits_truncate(flags.bits()),
            },
        )
    }

    fn protect_and_clone_region(
        &mut self,
        region: VirtMemoryRegion,
        other: &mut Self,
        perms: PtePermissions,
    ) -> Result<(), ()> {
        let start_va = region.start_va & !(PAGE_SIZE - 1);
        let end_va = (region.start_va + region.len + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let flags = UniversalPTEFlag::from_bits_truncate(perms.bits());

        for va in (start_va..end_va).step_by(PAGE_SIZE) {
            let vaddr = Vaddr::from_usize(va);
            let vpn = Vpn::from_addr_floor(vaddr);
            if let Ok((ppn, _size, _flags)) = self.inner.page_table().walk(vpn) {
                other
                    .inner
                    .page_table_mut()
                    .map(vpn, ppn, PageSize::Size4K, flags)
                    .map_err(|_| ())?;
                self.inner
                    .page_table_mut()
                    .update_flags(vpn, flags)
                    .map_err(|_| ())?;
            }
        }
        Ok(())
    }
}
