use alloc::collections::btree_map::BTreeMap;
use core::cmp::min;

use crate::arch::mm::TlbBatchContext;
use crate::config::PAGE_SIZE;
use crate::mm::address::{PA, PageNum, Ppn, UsizeConvert, Vpn, VpnRange};
use crate::mm::frame_allocator::{TrackedFrames, alloc_frame};
use crate::mm::memory_space::MmapFile;
use crate::mm::page_table::{
    self, ActivePageTableInner, PageSize, PageTableInner, UniversalPTEFlag,
};
use crate::uapi::mm::MapFlags;
use crate::{pr_err, pr_warn};

/// 映射策略类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapType {
    /// 直接映射（虚拟地址 = 物理地址 + VIRTUAL_BASE）
    Direct,
    /// 帧映射（从帧分配器分配）
    Framed,
    /// 保留地址范围（不建立页表映射）
    ///
    /// 用于实现 PROT_NONE（guard page / no-access VMA）语义：
    /// - mmap(PROT_NONE) 需要“成功占位”但不应该映射可访问页表项
    /// - mprotect(PROT_NONE) 会把原有页表映射解除并转为 Reserved
    Reserved,
}

/// 内存区域的类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaType {
    KernelText,   // 内核代码段
    KernelRodata, // 内核只读数据段
    KernelData,   // 内核数据段
    KernelStack,  // 内核栈
    KernelBss,    // 内核 BSS 段
    KernelHeap,   // 内核堆
    KernelMmio,   // 内核内存映射 I/O
    UserText,     // 用户代码段
    UserRodata,   // 用户只读数据段
    UserData,     // 用户数据段
    UserBss,      // 用户 BSS 段
    UserStack,    // 用户栈
    UserHeap,     // 用户堆
    UserMmap,     // 用户 mmap 匿名映射
}

/// 内存空间中的一个内存映射区域
#[derive(Debug)]
pub struct MappingArea {
    /// 此映射区域的虚拟页号范围
    vpn_range: VpnRange,

    /// 此映射区域的类型
    area_type: AreaType,

    /// 映射策略类型
    map_type: MapType,

    /// 此映射区域的权限（使用 UniversalPTEFlag 以提高性能）
    permission: UniversalPTEFlag,

    /// 用于帧映射区域的跟踪帧
    frames: BTreeMap<Vpn, TrackedFrames>,

    /// 文件映射信息（如果是文件映射）
    file: Option<MmapFile>,
}

mod file_ops;
mod map_ops;
mod resize_ops;
mod split_ops;
