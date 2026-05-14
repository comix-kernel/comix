//! LoongArch 内存子系统 VirtualMemory trait 实现
//!
//! 包含 `LoongArch64KernelAddressSpace` 和 `LoongArch64ProcessAddressSpace`。

use alloc::vec::Vec;

use crate::config::PAGE_SIZE;
use crate::hal::virtual_memory::{
    KernAddressSpace, PageFrame, PageInfo, PhysMemoryRegion, PtePermissions, UserAddressSpace,
    VirtMemoryRegion,
};
use crate::mm::address::{
    ConvertablePaddr, Paddr, PageNum, Ppn, UsizeConvert, Vaddr, Vpn, VpnRange,
};
use crate::mm::memory_space::{with_kernel_space, MemorySpace};
use crate::mm::page_table::PageTableInner;
use crate::mm::page_table::{PageSize, UniversalPTEFlag};

// ---- LoongArch64ProcessAddressSpace ----

/// LoongArch 进程地址空间
///
/// 包装 `MemorySpace` 以提供 `UserAddressSpace` trait 实现。
pub struct LoongArch64ProcessAddressSpace {
    inner: MemorySpace,
}

impl LoongArch64ProcessAddressSpace {
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

impl UserAddressSpace for LoongArch64ProcessAddressSpace {
    fn new() -> Result<Self, ()> {
        Ok(Self::new())
    }

    fn activate(&self) {
        let root_ppn = self.inner.root_ppn();
        crate::arch::mm::PageTableInner::activate(root_ppn);
    }

    fn deactivate(&self) {
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

// ---- LoongArch64KernelAddressSpace ----

/// LoongArch64 内核地址空间
///
/// 包装对全局内核地址空间的访问，提供 MMIO 和普通内存映射功能。
pub struct LoongArch64KernelAddressSpace;

impl LoongArch64KernelAddressSpace {
    pub const fn new() -> Self {
        Self
    }
}

impl KernAddressSpace for LoongArch64KernelAddressSpace {
    fn map_mmio(&mut self, region: PhysMemoryRegion) -> Result<usize, ()> {
        let pa = Paddr::from_usize(region.start_pa);
        let va = pa.to_vaddr();

        with_kernel_space(|ks| {
            let start_pa = region.start_pa & !(PAGE_SIZE - 1);
            let end_pa = region.start_pa + region.len;
            let mut cur_pa = start_pa;
            let mut cur_va = va.as_usize() & !(PAGE_SIZE - 1);

            while cur_pa < end_pa {
                let ppn = Ppn::from_addr_floor(Paddr::from_usize(cur_pa));
                let vpn = Vpn::from_addr_floor(Vaddr::from_usize(cur_va));
                ks.page_table_mut()
                    .map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::kernel_rw())
                    .map_err(|_| ())?;
                cur_pa += PAGE_SIZE;
                cur_va += PAGE_SIZE;
            }
            Ok(())
        })?;

        Ok(va.as_usize())
    }

    fn map_normal(
        &mut self,
        phys_range: PhysMemoryRegion,
        virt_range: VirtMemoryRegion,
        perms: PtePermissions,
    ) -> Result<(), ()> {
        with_kernel_space(|ks| {
            let start_pa = phys_range.start_pa & !(PAGE_SIZE - 1);
            let end_pa = phys_range.start_pa + phys_range.len;
            let start_va = virt_range.start_va & !(PAGE_SIZE - 1);
            let mut offset = 0usize;

            while start_pa + offset < end_pa {
                let ppn = Ppn::from_addr_floor(Paddr::from_usize(start_pa + offset));
                let vpn = Vpn::from_addr_floor(Vaddr::from_usize(start_va + offset));
                let flags = UniversalPTEFlag::from_bits_truncate(perms.bits());
                ks.page_table_mut()
                    .map(vpn, ppn, PageSize::Size4K, flags)
                    .map_err(|_| ())?;
                offset += PAGE_SIZE;
            }
            Ok(())
        })
    }
}

/// 兼容性别名模块
pub mod address_space {
    pub use super::LoongArch64ProcessAddressSpace;
}
pub mod mmu {
    pub use super::LoongArch64KernelAddressSpace;
}
