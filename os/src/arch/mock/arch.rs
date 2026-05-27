//! Mock 实现 — Arch trait + CpuOps + VirtualMemory + AddressSpace
//!
//! 提供在宿主（x86_64）上编译和测试架构无关代码所需的 mock 类型。
//! 这些 mock 实现仅在非目标架构上激活。

use alloc::vec::Vec;

#[cfg(not(any(target_arch = "riscv64", target_arch = "loongarch64")))]
use crate::arch::arch::Arch;
use crate::arch::cpu_ops::CpuOps;
use crate::arch::virtual_memory::{
    KernAddressSpace, PageFrame, PageInfo, PhysMemoryRegion, PtePermissions, UserAddressSpace,
    VirtMemoryRegion, VirtualMemory,
};
use crate::mm::page_table::PagingError;
use crate::sync::SpinLock;

// 在非目标架构上，MockArch 的 UserContext 应等于 arch::kernel::context::Context，
// 这样才能与 scheduler 传递的 Context 类型匹配。
#[cfg(not(any(target_arch = "riscv64", target_arch = "loongarch64")))]
pub type MockUserContext = super::kernel::context::Context;

// ============================================================================
// MockCpuOps
// ============================================================================

/// Mock CPU 实现 — 用于宿主测试
pub struct MockCpuOps;

impl CpuOps for MockCpuOps {
    fn id() -> usize {
        0
    }

    fn halt() -> ! {
        loop {
            core::hint::spin_loop();
        }
    }

    fn disable_interrupts() -> usize {
        0
    }

    fn restore_interrupt_state(_flags: usize) {}

    fn enable_interrupts() {}

    fn interrupts_enabled() -> bool {
        false
    }
}

// ============================================================================
// Mock 地址空间
// ============================================================================

/// Mock 地址空间（同时实现 UserAddressSpace 和 KernAddressSpace）
pub struct MockAddressSpace {
    pub mappings: Vec<(usize, PageFrame, PtePermissions)>,
}

impl MockAddressSpace {
    pub const fn new() -> Self {
        Self {
            mappings: Vec::new(),
        }
    }
}

impl UserAddressSpace for MockAddressSpace {
    fn new() -> Result<Self, PagingError> {
        Ok(Self::new())
    }

    fn activate(&self) {}

    fn deactivate(&self) {}

    fn map_page(
        &mut self,
        page: PageFrame,
        va: usize,
        perms: PtePermissions,
    ) -> Result<(), PagingError> {
        self.mappings.push((va, page, perms));
        Ok(())
    }

    fn unmap(&mut self, va: usize) -> Result<PageFrame, PagingError> {
        if let Some(pos) = self.mappings.iter().position(|(v, _, _)| *v == va) {
            let (_, frame, _) = self.mappings.remove(pos);
            Ok(frame)
        } else {
            Err(PagingError::NotMapped)
        }
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
        _region: VirtMemoryRegion,
        _perms: PtePermissions,
    ) -> Result<(), PagingError> {
        Ok(())
    }

    fn unmap_range(&mut self, region: VirtMemoryRegion) -> Result<Vec<PageFrame>, PagingError> {
        let start = region.start_va;
        let end = start + region.len;
        let frames: Vec<_> = self
            .mappings
            .iter()
            .filter(|(va, _, _)| *va >= start && *va < end)
            .map(|(_, frame, _)| *frame)
            .collect();
        self.mappings.retain(|(va, _, _)| *va < start || *va >= end);
        Ok(frames)
    }

    fn translate(&self, va: usize) -> Option<PageInfo> {
        self.mappings
            .iter()
            .find(|(v, _, _)| *v == va)
            .map(|(_, frame, perms)| PageInfo {
                ppn: frame.ppn,
                perms: *perms,
            })
    }

    fn protect_and_clone_region(
        &mut self,
        region: VirtMemoryRegion,
        _other: &mut Self,
        _perms: PtePermissions,
    ) -> Result<(), PagingError> {
        self.protect_range(region, _perms)
    }
}

impl KernAddressSpace for MockAddressSpace {
    fn map_mmio(&mut self, _region: PhysMemoryRegion) -> Result<usize, PagingError> {
        Ok(0)
    }

    fn map_normal(
        &mut self,
        _phys_range: PhysMemoryRegion,
        _virt_range: VirtMemoryRegion,
        _perms: PtePermissions,
    ) -> Result<(), PagingError> {
        Ok(())
    }
}

// ============================================================================
// MockArch
// ============================================================================

pub struct MockArch;

impl CpuOps for MockArch {
    fn id() -> usize {
        MockCpuOps::id()
    }
    fn halt() -> ! {
        MockCpuOps::halt()
    }
    fn disable_interrupts() -> usize {
        MockCpuOps::disable_interrupts()
    }
    fn restore_interrupt_state(flags: usize) {
        MockCpuOps::restore_interrupt_state(flags)
    }
    fn enable_interrupts() {
        MockCpuOps::enable_interrupts()
    }
    fn interrupts_enabled() -> bool {
        MockCpuOps::interrupts_enabled()
    }
}

impl VirtualMemory for MockArch {
    type PageTableRoot = ();
    type ProcessAddressSpace = MockAddressSpace;
    type KernelAddressSpace = MockAddressSpace;
    const PAGE_OFFSET: usize = 0xffff_ffc0_0000_0000;
    const USER_TOP: usize = super::constant::USER_TOP;

    fn kern_address_space() -> &'static SpinLock<Self::KernelAddressSpace> {
        static KERN_SPACE: SpinLock<MockAddressSpace> = SpinLock::new(MockAddressSpace::new());
        &KERN_SPACE
    }
}

#[cfg(not(any(target_arch = "riscv64", target_arch = "loongarch64")))]
mod mock_arch_impl {
    use super::*;
    use crate::arch::plat::Platform;

    impl Arch for MockArch {
        type UserContext = MockUserContext;

        fn new_user_context(entry_point: usize, stack_top: usize) -> Self::UserContext {
            let mut ctx = MockUserContext::zero_init();
            ctx.set_init_context(entry_point, stack_top);
            ctx
        }

        unsafe fn context_switch(_old: *mut Self::UserContext, _new: *const Self::UserContext) {}

        unsafe fn copy_from_user(
            src: crate::arch::address::UA,
            dst: *mut u8,
            len: usize,
        ) -> Result<(), PagingError> {
            let src = src.as_usize();
            if len != 0 && (src == 0 || dst.is_null()) {
                return Err(PagingError::InvalidAddress);
            }
            unsafe { core::ptr::copy_nonoverlapping(src as *const u8, dst, len) };
            Ok(())
        }

        unsafe fn try_copy_from_user(
            src: crate::arch::address::UA,
            dst: *mut u8,
            len: usize,
        ) -> Result<(), PagingError> {
            unsafe { Self::copy_from_user(src, dst, len) }
        }

        unsafe fn copy_to_user(
            src: *const u8,
            dst: crate::arch::address::UA,
            len: usize,
        ) -> Result<(), PagingError> {
            let dst = dst.as_usize();
            if len != 0 && (src.is_null() || dst == 0) {
                return Err(PagingError::InvalidAddress);
            }
            unsafe { core::ptr::copy_nonoverlapping(src, dst as *mut u8, len) };
            Ok(())
        }

        unsafe fn copy_strn_from_user(
            src: crate::arch::address::UA,
            dst: *mut u8,
            max_len: usize,
        ) -> Result<usize, PagingError> {
            let src = src.as_usize();
            if max_len != 0 && (src == 0 || dst.is_null()) {
                return Err(PagingError::InvalidAddress);
            }
            let mut i = 0;
            while i < max_len {
                let byte = unsafe { core::ptr::read((src + i) as *const u8) };
                unsafe { *dst.add(i) = byte };
                if byte == 0 {
                    return Ok(i);
                }
                i += 1;
            }
            Ok(max_len)
        }

        fn name() -> &'static str {
            "mock"
        }

        fn cpu_count() -> usize {
            1
        }

        fn on_task_switch(_trap_frame_ptr: usize, _cpu_ptr: usize) {}

        fn get_ticks() -> usize {
            0
        }

        fn get_time() -> usize {
            0
        }

        fn get_time_ms() -> usize {
            0
        }

        fn clock_freq() -> usize {
            12_500_000
        }

        fn send_reschedule_ipi(_target_cpu: usize) {}
    }

    impl Platform for MockArch {
        fn console_putchar(_c: u8) {}

        fn console_getchar() -> Option<u8> {
            None
        }

        fn get_cmdline() -> Option<alloc::string::String> {
            None
        }

        fn power_off() -> ! {
            loop {
                core::hint::spin_loop();
            }
        }

        fn restart() -> ! {
            loop {
                core::hint::spin_loop();
            }
        }
    }
}
