#![allow(dead_code)]
//! 内存管理模块
//!
//! 本模块为内核提供与体系结构无关的内存管理抽象和实现。
//!
//! # 模块组成
//!
//! - [`address`]：地址和页号抽象。
//! - [`frame_allocator`]：物理帧分配。
//! - [`mod@global_allocator`]：全局堆分配器。
//! - [`memory_space`]：内存空间管理。
//! - [`page_table`]：页表抽象和实现（与架构无关）。

pub mod address;
pub mod frame_allocator;
pub mod global_allocator;
pub mod memory_space;
pub mod page_table;

pub use frame_allocator::init_frame_allocator;
pub use global_allocator::init_heap;

use crate::arch::mm::vaddr_to_paddr;
use crate::config::{MEMORY_END, PAGE_SIZE};
use crate::earlyprintln;
use crate::mm::address::{Ppn, UsizeConvert};
use crate::sync::SpinLock;
use alloc::sync::Arc;

unsafe extern "C" {
    // 链接器脚本中定义的内核结束地址
    fn ekernel();
}

/// 初始化内存管理子系统
///
/// 此函数执行所有内存管理组件的初始化工作：
/// 1. 初始化物理帧分配器。
/// 2. 初始化内核堆分配器。
/// 3. 创建内核地址空间（不激活，由调用者在合适时机激活）。
///
/// # 返回值
/// 返回创建的内核地址空间，调用者需要在合适时机激活它。
pub fn init() -> alloc::sync::Arc<crate::sync::SpinLock<memory_space::MemorySpace>> {
    // 1. 初始化物理帧分配器

    // ekernel 是一个虚拟地址，需要转换为物理地址，以确定可分配物理内存的起始点。
    let ekernel_paddr = unsafe { vaddr_to_paddr(ekernel as usize) };

    // 计算页对齐后的物理内存起始地址。
    // 分配器将管理 [start, end) 范围内的内存。
    let start = ekernel_paddr.div_ceil(PAGE_SIZE) * PAGE_SIZE; // 页对齐

    let end = MEMORY_END;

    // 初始化物理帧分配器
    init_frame_allocator(start, end);

    // 2. 初始化堆分配器
    init_heap();

    // 3. 创建内核地址空间（不激活，由调用者在合适时机激活）
    let space = Arc::new(SpinLock::new(memory_space::MemorySpace::new_kernel()));

    // 记录全局内核空间句柄，供次核切换使用（确保所有 CPU 使用同一份内核页表）
    set_global_kernel_space(space.clone());

    let root_ppn = space.lock().root_ppn();
    earlyprintln!(
        "[MM] Created kernel space, root PPN: 0x{:x}",
        root_ppn.as_usize()
    );
    space
}

/// 激活指定的地址空间
///
/// 通过将根页表（Page Table Root）的物理页号写入特定的寄存器，
/// 从而切换当前 CPU 使用的地址空间。
///
/// # 参数
///
/// * `root_ppn` - 新地址空间的根页表的物理页号。
pub fn activate(root_ppn: Ppn) {
    use crate::mm::page_table::PageTableInner as PageTableInnerTrait;
    // 调用特定架构的页表激活函数，例如在 RISC-V 上设置 SATP 寄存器。
    crate::arch::mm::PageTableInner::activate(root_ppn);
}

// === 全局内核空间句柄（供所有 CPU 共享同一内核页表） ===

/// 保存 CPU0 创建的最终内核页表（MemorySpace）的共享句柄。
///
/// 说明：
/// - 仅在启动阶段由 `mm::init()` 设置一次。
/// - 其他 CPU 在启动时（secondary_start）应当从这里获取并切换到该页表，
///   确保所有 CPU 的内核映射完全一致，避免早期页表（boot_pagetable）长期驻留引发不一致。
static GLOBAL_KERNEL_SPACE: SpinLock<Option<Arc<SpinLock<memory_space::MemorySpace>>>> =
    SpinLock::new(None);

/// 由 CPU0 在初始化完成时设置全局内核空间。
pub fn set_global_kernel_space(space: Arc<SpinLock<memory_space::MemorySpace>>) {
    let mut g = GLOBAL_KERNEL_SPACE.lock();
    *g = Some(space);
}

/// 获取全局内核空间句柄（如果已初始化）。
pub fn get_global_kernel_space() -> Option<Arc<SpinLock<memory_space::MemorySpace>>> {
    GLOBAL_KERNEL_SPACE.lock().as_ref().cloned()
}
