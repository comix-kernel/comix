//! VirtualMemory / UserAddressSpace / KernAddressSpace trait 定义
//!
//! 这些 trait 抽象了页表操作、地址空间管理和内存映射。

use alloc::vec::Vec;

use crate::arch::cpu_ops::CpuOps;
use crate::sync::SpinLock;

// ============================================================================
// 页表权限标志
// ============================================================================

bitflags::bitflags! {
    /// 页表项权限标志（架构无关表示）
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PtePermissions: usize {
        /// 有效
        const VALID = 1 << 0;
        /// 可读
        const READABLE = 1 << 1;
        /// 可写
        const WRITEABLE = 1 << 2;
        /// 可执行
        const EXECUTABLE = 1 << 3;
        /// 用户可访问
        const USER_ACCESSIBLE = 1 << 4;
        /// 全局页
        const GLOBAL = 1 << 5;
        /// 已访问
        const ACCESSED = 1 << 6;
        /// 已修改（脏）
        const DIRTY = 1 << 7;

        // 便捷组合
        const USER_R = Self::VALID.bits() | Self::READABLE.bits() | Self::USER_ACCESSIBLE.bits();
        const USER_RW = Self::USER_R.bits() | Self::WRITEABLE.bits();
        const USER_RX = Self::USER_R.bits() | Self::EXECUTABLE.bits();
        const KERNEL_RW = Self::VALID.bits() | Self::READABLE.bits() | Self::WRITEABLE.bits();
        const KERNEL_R = Self::VALID.bits() | Self::READABLE.bits();
        const KERNEL_RX = Self::KERNEL_R.bits() | Self::EXECUTABLE.bits();
    }
}

// ============================================================================
// 辅助类型
// ============================================================================

/// 页帧信息（物理页号 + 权限）
#[derive(Debug, Clone, Copy)]
pub struct PageInfo {
    /// 物理页号（以 `usize` 表示）
    pub ppn: usize,
    /// 页表项权限
    pub perms: PtePermissions,
}

/// 页帧（物理页号）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFrame {
    pub ppn: usize,
}

/// 虚拟内存区域描述
#[derive(Debug, Clone, Copy)]
pub struct VirtMemoryRegion {
    pub start_va: usize,
    pub len: usize,
}

/// 物理内存区域描述
#[derive(Debug, Clone, Copy)]
pub struct PhysMemoryRegion {
    pub start_pa: usize,
    pub len: usize,
}

// ============================================================================
// UserAddressSpace — 进程地址空间 trait
// ============================================================================

/// 用户地址空间抽象。
///
/// 这个 trait 完全解耦了进程地址空间的操作。架构相关实现只需填充方法。
pub trait UserAddressSpace: Send + Sync {
    /// 新建空页表
    fn new() -> Result<Self, ()>
    where
        Self: Sized;

    /// 激活此地址空间（写入 TTBR0 / SATP 等寄存器）
    fn activate(&self);

    /// 反激活
    fn deactivate(&self);

    /// 映射一页
    fn map_page(&mut self, page: PageFrame, va: usize, perms: PtePermissions) -> Result<(), ()>;

    /// 取消映射一页，返回被解除的页帧
    fn unmap(&mut self, va: usize) -> Result<PageFrame, ()>;

    /// 重新映射一页（替换）
    fn remap(
        &mut self,
        va: usize,
        new_page: PageFrame,
        perms: PtePermissions,
    ) -> Result<PageFrame, ()>;

    /// 保护一个内存范围
    fn protect_range(&mut self, region: VirtMemoryRegion, perms: PtePermissions) -> Result<(), ()>;

    /// 取消映射一个内存范围，返回被解除的所有页帧
    fn unmap_range(&mut self, region: VirtMemoryRegion) -> Result<Vec<PageFrame>, ()>;

    /// 翻译虚拟地址 → 页信息
    fn translate(&self, va: usize) -> Option<PageInfo>;

    /// 保护并克隆一个区域到另一个地址空间（用于 CoW fork）
    fn protect_and_clone_region(
        &mut self,
        region: VirtMemoryRegion,
        other: &mut Self,
        perms: PtePermissions,
    ) -> Result<(), ()>;
}

// ============================================================================
// KernAddressSpace — 内核地址空间 trait
// ============================================================================

/// 内核地址空间抽象。
pub trait KernAddressSpace: Send {
    /// 映射 MMIO 区域
    fn map_mmio(&mut self, region: PhysMemoryRegion) -> Result<usize, ()>;

    /// 映射普通内存区域
    fn map_normal(
        &mut self,
        phys_range: PhysMemoryRegion,
        virt_range: VirtMemoryRegion,
        perms: PtePermissions,
    ) -> Result<(), ()>;
}

// ============================================================================
// VirtualMemory — 内存子系统 trait
// ============================================================================

/// 虚拟内存子系统抽象。
///
/// 此 trait 组合了 `CpuOps`，建立内存子系统与 CPU 操作的关联。
pub trait VirtualMemory: CpuOps + Sized {
    /// 顶层页表类型
    type PageTableRoot;

    /// 进程地址空间类型
    type ProcessAddressSpace: UserAddressSpace;

    /// 内核地址空间类型
    type KernelAddressSpace: KernAddressSpace;

    /// 物理 → 虚拟映射偏移
    ///
    /// 对于直接映射，`paddr + PAGE_OFFSET = vaddr`。
    const PAGE_OFFSET: usize;

    /// 用户地址空间最高可访问地址
    const USER_TOP: usize;

    /// 获取全局内核地址空间
    fn kern_address_space() -> &'static SpinLock<Self::KernelAddressSpace>;
}
