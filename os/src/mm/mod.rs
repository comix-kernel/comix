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
use crate::mm::address::{Ppn, UsizeConvert};
use crate::println;

unsafe extern "C" {
    // 链接器脚本中定义的内核结束地址
    fn ekernel();
}

/// 初始化内存管理子系统
///
/// 此函数执行所有内存管理组件的初始化工作：
/// 1. 初始化物理帧分配器。
/// 2. 初始化内核堆分配器。
/// 3. 创建并激活内核地址空间。
pub fn init() {
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

    // 3. 创建并激活内核地址空间 (内核地址空间通过 lazy_static 自动初始化)
    #[cfg(target_arch = "riscv64")]
    {
        use alloc::sync::Arc;

        use crate::{
            earlyprintln, kernel::current_cpu, mm::memory_space::MemorySpace, sync::SpinLock,
        };

        // 记录切换前的 satp 值
        let old_satp: usize;
        unsafe {
            core::arch::asm!("csrr {0}, satp", out(reg) old_satp);
        }
        earlyprintln!("[MM] Before space switch - satp: 0x{:x}", old_satp);

        let space = Arc::new(SpinLock::new(MemorySpace::new_kernel()));
        let root_ppn = space.lock().root_ppn();
        earlyprintln!(
            "[MM] New kernel space root PPN: 0x{:x}",
            root_ppn.as_usize()
        );

        current_cpu().lock().switch_space(space);

        // 记录切换后的 satp 值
        let new_satp: usize;
        unsafe {
            core::arch::asm!("csrr {0}, satp", out(reg) new_satp);
        }
        earlyprintln!("[MM] After space switch - satp: 0x{:x}", new_satp);
        earlyprintln!(
            "[MM] Expected satp: 0x{:x}",
            (root_ppn.as_usize() | (8 << 60))
        );
    }
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
