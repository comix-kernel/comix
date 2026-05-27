use core::cmp::Ordering;

use crate::arch::platform::MEMORY_END;
use crate::config::{
    MAX_USER_HEAP_SIZE, PAGE_SIZE, USER_SIGRETURN_TRAMPOLINE, USER_STACK_SIZE, USER_STACK_TOP,
};
use crate::mm::address::{PA, PageNum, Ppn, UsizeConvert, VA, Vpn, VpnRange};
use crate::mm::memory_space::MmapFile;
use crate::mm::memory_space::mapping_area::{AreaType, MapType, MappingArea};
use crate::mm::page_table::{ActivePageTableInner, PageTableInner, PagingError, UniversalPTEFlag};
use crate::sync::SpinLock;
use crate::{pr_err, pr_warn};
use alloc::vec::Vec;
use lazy_static::lazy_static;

// 内核链接器符号
unsafe extern "C" {
    fn stext(); // .text (代码段) 的起始地址
    fn etext(); // .text (代码段) 的结束地址
    fn srodata(); // .rodata (只读数据段) 的起始地址
    fn erodata(); // .rodata (只读数据段) 的结束地址
    fn sdata(); // .data (数据段) 的起始地址
    fn edata(); // .data (数据段) 的结束地址
    fn sbss(); // .bss (未初始化数据段) 的起始地址
    fn ebss(); // .bss (未初始化数据段) 的结束地址
    fn ekernel(); // 内核所有段的结束地址（即物理内存的起始可分配地址）
    fn strampoline(); // 位于高半部分的跳板页 (trampoline page) 的起始地址
}

lazy_static! {
    /// 全局内核内存空间（受 SpinLock 保护）
    static ref KERNEL_SPACE: SpinLock<MemorySpace> = {
        SpinLock::new(MemorySpace::new_kernel().expect("failed to create kernel memory space"))
    };
}

/// 返回内核页表令牌（用于激活页表，例如 RISC-V 上的 satp 寄存器值）
pub fn kernel_token() -> usize {
    (KERNEL_SPACE.lock().page_table.root_ppn().as_usize() << 44) | (8 << 60)
}

/// 返回内核根页表的物理页号 (PPN)
pub fn kernel_root_ppn() -> Ppn {
    KERNEL_SPACE.lock().root_ppn()
}

/// 以独占方式访问内核空间并执行闭包
pub fn with_kernel_space<F, R>(f: F) -> R
where
    F: FnOnce(&mut MemorySpace) -> R,
{
    let mut guard = KERNEL_SPACE.lock();
    f(&mut guard)
}

/// 表示地址空间的内存空间结构体
#[derive(Debug)]
pub struct MemorySpace {
    /// 与此内存空间关联的页表
    page_table: ActivePageTableInner,

    /// 此内存空间中的映射区域列表
    areas: Vec<MappingArea>,

    /// 堆的起始地址 (brk 系统调用使用，仅限用户空间)
    /// 注意：这是堆的固定起始位置，真正的堆顶（current brk）存储在 UserHeap 区域的 vpn_range.end 中
    heap_start: Option<Vpn>,
}

mod address_space;
mod elf_loader;
mod kernel_space;
mod mmap_ops;
#[cfg(test)]
mod tests;
