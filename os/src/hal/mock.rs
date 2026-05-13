//! Mock 实现（仅 test 编译时可用）
//!
//! 提供 `MockCpuOps`、`MockAddressSpace`、`MockArch` 等，
//! 使得架构无关代码可以在宿主编译和测试。
//!
//! 这些 mock 实现不会出现在非测试构建中。

use alloc::vec::Vec;

use crate::hal::arch::Arch;
use crate::hal::cpu_ops::CpuOps;
use crate::hal::virtual_memory::{
    KernAddressSpace, PageFrame, PageInfo, PhysMemoryRegion, PtePermissions, UserAddressSpace,
    VirtualMemory, VirtMemoryRegion,
};
use crate::sync::SpinLock;

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
    fn new() -> Result<Self, ()> {
        Ok(Self::new())
    }

    fn activate(&self) {}

    fn deactivate(&self) {}

    fn map_page(&mut self, page: PageFrame, va: usize, perms: PtePermissions) -> Result<(), ()> {
        self.mappings.push((va, page, perms));
        Ok(())
    }

    fn unmap(&mut self, va: usize) -> Result<PageFrame, ()> {
        if let Some(pos) = self.mappings.iter().position(|(v, _, _)| *v == va) {
            let (_, frame, _) = self.mappings.remove(pos);
            Ok(frame)
        } else {
            Err(())
        }
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
        _region: VirtMemoryRegion,
        _perms: PtePermissions,
    ) -> Result<(), ()> {
        Ok(())
    }

    fn unmap_range(&mut self, region: VirtMemoryRegion) -> Result<Vec<PageFrame>, ()> {
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
    ) -> Result<(), ()> {
        self.protect_range(region, _perms)
    }
}

impl KernAddressSpace for MockAddressSpace {
    fn map_mmio(&mut self, _region: PhysMemoryRegion) -> Result<usize, ()> {
        Ok(0)
    }

    fn map_normal(
        &mut self,
        _phys_range: PhysMemoryRegion,
        _virt_range: VirtMemoryRegion,
        _perms: PtePermissions,
    ) -> Result<(), ()> {
        Ok(())
    }
}

// ============================================================================
// MockArch
// ============================================================================

/// Mock 用户上下文
#[derive(Debug, Clone)]
pub struct MockUserContext {
    pub entry_point: usize,
    pub stack_top: usize,
}

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
}

impl VirtualMemory for MockArch {
    type PageTableRoot = ();
    type ProcessAddressSpace = MockAddressSpace;
    type KernelAddressSpace = MockAddressSpace;
    const PAGE_OFFSET: usize = 0xffff_ffc0_0000_0000;

    fn kern_address_space() -> &'static SpinLock<Self::KernelAddressSpace> {
        static KERN_SPACE: SpinLock<MockAddressSpace> = SpinLock::new(MockAddressSpace::new());
        &KERN_SPACE
    }
}

impl Arch for MockArch {
    type UserContext = MockUserContext;

    fn new_user_context(entry_point: usize, stack_top: usize) -> Self::UserContext {
        MockUserContext {
            entry_point,
            stack_top,
        }
    }

    unsafe fn context_switch(_new_ctx: &Self::UserContext) {}

    unsafe fn copy_from_user(_src: usize, _dst: *mut u8, _len: usize) -> Result<(), ()> {
        Ok(())
    }

    unsafe fn try_copy_from_user(_src: usize, _dst: *mut u8, _len: usize) -> Result<(), ()> {
        Ok(())
    }

    unsafe fn copy_to_user(_src: *const u8, _dst: usize, _len: usize) -> Result<(), ()> {
        Ok(())
    }

    unsafe fn copy_strn_from_user(
        _src: usize,
        _dst: *mut u8,
        _max_len: usize,
    ) -> Result<usize, ()> {
        Ok(0)
    }

    fn name() -> &'static str {
        "mock"
    }

    fn cpu_count() -> usize {
        1
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
