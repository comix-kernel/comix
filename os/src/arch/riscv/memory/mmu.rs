//! RISC-V 内核地址空间 (KernAddressSpace) 实现

use crate::config::PAGE_SIZE;
use crate::hal::virtual_memory::{KernAddressSpace, PhysMemoryRegion, PtePermissions, VirtMemoryRegion};
use crate::mm::address::{ConvertablePaddr, Paddr, PageNum, Ppn, UsizeConvert, Vaddr, Vpn};
use crate::mm::memory_space::with_kernel_space;
use crate::mm::page_table::PageTableInner;
use crate::mm::page_table::{PageSize, UniversalPTEFlag};

/// RISC-V 内核地址空间
///
/// 包装对全局内核地址空间的访问，提供 MMIO 和普通内存映射功能。
pub struct Riscv64KernelAddressSpace;

impl Riscv64KernelAddressSpace {
    pub const fn new() -> Self {
        Self
    }
}

impl KernAddressSpace for Riscv64KernelAddressSpace {
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
