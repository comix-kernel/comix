//! VirtualMemory 实现生成宏
//!
//! 为不同架构生成 `UserAddressSpace` 和 `KernAddressSpace` 的实现。
//! 两个架构的实现完全相同，仅类型名不同。

/// 为指定架构生成 `ProcessAddressSpace` 和 `KernelAddressSpace`。
///
/// 用法：`impl_virtual_memory!(Riscv64ProcessAddressSpace, Riscv64KernelAddressSpace);`
#[macro_export]
macro_rules! impl_virtual_memory {
    ($process_type:ident, $kernel_type:ident) => {
        use alloc::vec::Vec;

        use crate::mm::address::{
            ConvertablePA, PA, PageNum, Ppn, UsizeConvert, VA, Vpn, VpnRange,
        };
        use crate::mm::memory_space::{MemorySpace, with_kernel_space};
        use crate::mm::page_table::PageTableInner;
        use crate::mm::page_table::{PageSize, PagingError, UniversalPTEFlag};
        use $crate::arch::virtual_memory::{
            KernAddressSpace, PageFrame, PageInfo, PhysMemoryRegion, PtePermissions,
            UserAddressSpace, VirtMemoryRegion,
        };
        use $crate::config::PAGE_SIZE;

        pub struct $process_type {
            #[allow(dead_code)]
            inner: MemorySpace,
        }

        #[allow(dead_code)]
        impl $process_type {
            pub fn new() -> Result<Self, PagingError> {
                Ok(Self {
                    inner: MemorySpace::new()?,
                })
            }

            pub fn inner(&self) -> &MemorySpace {
                &self.inner
            }

            pub fn inner_mut(&mut self) -> &mut MemorySpace {
                &mut self.inner
            }
        }

        impl UserAddressSpace for $process_type {
            fn new() -> Result<Self, PagingError> {
                Self::new()
            }

            fn activate(&self) {
                let root_ppn = self.inner.root_ppn();
                crate::arch::mm::PageTableInner::activate(root_ppn);
            }

            fn deactivate(&self) {}

            fn map_page(
                &mut self,
                page: PageFrame,
                va: usize,
                perms: PtePermissions,
            ) -> Result<(), PagingError> {
                let ppn = Ppn::from_usize(page.ppn);
                let vaddr = VA::from_usize(va);
                let vpn = Vpn::from_addr_floor(vaddr);
                let flags = UniversalPTEFlag::from_bits_truncate(perms.bits());
                self.inner
                    .page_table_mut()
                    .map(vpn, ppn, PageSize::Size4K, flags)
            }

            fn unmap(&mut self, va: usize) -> Result<PageFrame, PagingError> {
                let vaddr = VA::from_usize(va);
                let vpn = Vpn::from_addr_floor(vaddr);
                let (ppn, _size, _flags) = self.inner.page_table().walk(vpn)?;
                self.inner.page_table_mut().unmap(vpn)?;
                Ok(PageFrame {
                    ppn: ppn.as_usize(),
                })
            }

            fn remap(
                &mut self,
                va: usize,
                new_page: PageFrame,
                perms: PtePermissions,
            ) -> Result<PageFrame, PagingError> {
                let old = self.unmap(va)?;
                self.map_page(new_page, va, perms)?;
                Ok(old)
            }

            fn protect_range(
                &mut self,
                region: VirtMemoryRegion,
                perms: PtePermissions,
            ) -> Result<(), PagingError> {
                let start_vpn = Vpn::from_addr_floor(VA::from_usize(region.start_va));
                let end_vpn = Vpn::from_addr_ceil(VA::from_usize(region.start_va + region.len));
                let flags = UniversalPTEFlag::from_bits_truncate(perms.bits());

                for vpn in VpnRange::new(start_vpn, end_vpn) {
                    self.inner.page_table_mut().update_flags(vpn, flags)?;
                }
                Ok(())
            }

            fn unmap_range(
                &mut self,
                region: VirtMemoryRegion,
            ) -> Result<Vec<PageFrame>, PagingError> {
                let start_vpn = Vpn::from_addr_floor(VA::from_usize(region.start_va));
                let end_vpn = Vpn::from_addr_ceil(VA::from_usize(region.start_va + region.len));
                let range = VpnRange::new(start_vpn, end_vpn);

                let mut frames = Vec::new();
                for vpn in range {
                    if let Ok((ppn, _size, _flags)) = self.inner.page_table().walk(vpn) {
                        self.inner.page_table_mut().unmap(vpn)?;
                        frames.push(PageFrame {
                            ppn: ppn.as_usize(),
                        });
                    }
                }
                Ok(frames)
            }

            fn translate(&self, va: usize) -> Option<PageInfo> {
                let vaddr = VA::from_usize(va);
                let vpn = Vpn::from_addr_floor(vaddr);
                self.inner
                    .page_table()
                    .walk(vpn)
                    .ok()
                    .map(|(ppn, _size, flags)| PageInfo {
                        ppn: ppn.as_usize(),
                        perms: PtePermissions::from_bits_truncate(flags.bits()),
                    })
            }

            fn protect_and_clone_region(
                &mut self,
                region: VirtMemoryRegion,
                other: &mut Self,
                perms: PtePermissions,
            ) -> Result<(), PagingError> {
                let start_va = region.start_va & !(PAGE_SIZE - 1);
                let end_va = (region.start_va + region.len + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
                let flags = UniversalPTEFlag::from_bits_truncate(perms.bits());

                for va in (start_va..end_va).step_by(PAGE_SIZE) {
                    let vaddr = VA::from_usize(va);
                    let vpn = Vpn::from_addr_floor(vaddr);
                    if let Ok((ppn, _size, _flags)) = self.inner.page_table().walk(vpn) {
                        other
                            .inner
                            .page_table_mut()
                            .map(vpn, ppn, PageSize::Size4K, flags)?;
                        self.inner.page_table_mut().update_flags(vpn, flags)?;
                    }
                }
                Ok(())
            }
        }

        pub struct $kernel_type;

        #[allow(dead_code)]
        impl $kernel_type {
            pub const fn new() -> Self {
                Self
            }
        }

        impl KernAddressSpace for $kernel_type {
            fn map_mmio(&mut self, region: PhysMemoryRegion) -> Result<usize, PagingError> {
                let pa = PA::from_usize(region.start_pa);
                let va = pa.to_va();

                with_kernel_space(|ks| {
                    let start_pa = region.start_pa & !(PAGE_SIZE - 1);
                    let end_pa = region.start_pa + region.len;
                    let mut cur_pa = start_pa;
                    let mut cur_va = va.as_usize() & !(PAGE_SIZE - 1);

                    while cur_pa < end_pa {
                        let ppn = Ppn::from_addr_floor(PA::from_usize(cur_pa));
                        let vpn = Vpn::from_addr_floor(VA::from_usize(cur_va));
                        ks.page_table_mut().map(
                            vpn,
                            ppn,
                            PageSize::Size4K,
                            UniversalPTEFlag::kernel_rw(),
                        )?;
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
            ) -> Result<(), PagingError> {
                with_kernel_space(|ks| {
                    let start_pa = phys_range.start_pa & !(PAGE_SIZE - 1);
                    let end_pa = phys_range.start_pa + phys_range.len;
                    let start_va = virt_range.start_va & !(PAGE_SIZE - 1);
                    let mut offset = 0usize;

                    while start_pa + offset < end_pa {
                        let ppn = Ppn::from_addr_floor(PA::from_usize(start_pa + offset));
                        let vpn = Vpn::from_addr_floor(VA::from_usize(start_va + offset));
                        let flags = UniversalPTEFlag::from_bits_truncate(perms.bits());
                        ks.page_table_mut().map(vpn, ppn, PageSize::Size4K, flags)?;
                        offset += PAGE_SIZE;
                    }
                    Ok(())
                })
            }
        }

        #[allow(unused_imports)]
        pub mod address_space {
            pub use super::$process_type;
        }
        #[allow(unused_imports)]
        pub mod mmu {
            pub use super::$kernel_type;
        }
    };
}
