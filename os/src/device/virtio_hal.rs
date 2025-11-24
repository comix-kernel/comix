//! HAL (硬件抽象层) 实现，用于适配 virtio-drivers 0.12.0 库

use crate::arch::mm::{paddr_to_vaddr, vaddr_to_paddr};
use crate::mm::address::{ConvertablePaddr, PageNum, UsizeConvert};
use crate::mm::frame_allocator::FrameRangeTracker;
use crate::println;
use crate::sync::SpinLock;
use alloc::collections::btree_map::BTreeMap;
use core::ptr::NonNull;
use lazy_static::lazy_static;
use virtio_drivers::{BufferDirection, Hal, PhysAddr};

// 全局映射表，用于跟踪物理地址到分配的帧范围的映射
lazy_static! {
    static ref DMA_ALLOCATIONS: SpinLock<BTreeMap<PhysAddr, FrameRangeTracker>> =
        SpinLock::new(BTreeMap::new());
}

/// virtio-drivers 0.12.0 库使用的 HAL 实现
pub struct VirtIOHal;

unsafe impl Hal for VirtIOHal {
    /// 分配并清零指定数量的连续物理页用于DMA
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        println!("[VirtIOHal] dma_alloc called: requesting {} pages", pages);
        println!("[VirtIOHal] About to call alloc_contig_frames...");

        // 使用系统的连续物理帧分配器
        println!("[VirtIOHal] Calling frame allocator...");
        let frame_range = match crate::mm::frame_allocator::alloc_contig_frames(pages) {
            Some(range) => {
                println!(
                    "[VirtIOHal] Successfully allocated {} contiguous frames",
                    pages
                );
                range
            }
            None => {
                println!("[VirtIOHal] Failed to allocate {} contiguous frames", pages);
                // 返回空指针，让上层代码处理错误
                return (PhysAddr::from(0u64), NonNull::dangling());
            }
        };

        // 获取起始物理页号
        let start_ppn = frame_range.start_ppn();
        println!("[VirtIOHal] Start PPN: 0x{:x}", start_ppn.as_usize());

        // 计算物理地址
        let phys_addr = PhysAddr::from(start_ppn.start_addr().as_usize() as u64);
        println!("[VirtIOHal] Physical address: 0x{:x}", phys_addr);

        // 将物理地址转换为虚拟地址
        let virt_addr = unsafe { start_ppn.start_addr().to_vaddr().as_mut_ptr::<u8>() };
        let virt_ptr = NonNull::new(virt_addr).unwrap();
        println!("[VirtIOHal] Virtual address: 0x{:x}", virt_addr as usize);

        // 清零 DMA 缓冲区（VirtIO HAL trait 要求）
        println!(
            "[VirtIOHal] Starting to zero {} bytes at VA: 0x{:x}",
            pages * crate::config::PAGE_SIZE,
            virt_addr as usize
        );

        // 添加诊断：逐页测试写入并清零
        println!("[VirtIOHal] Clearing memory page by page...");
        unsafe {
            for page_idx in 0..pages {
                let page_start = virt_addr.add(page_idx * crate::config::PAGE_SIZE);
                println!(
                    "[VirtIOHal] Clearing page {} at VA: 0x{:x}",
                    page_idx, page_start as usize
                );

                // 逐字节清零整个页面
                for offset in 0..crate::config::PAGE_SIZE {
                    core::ptr::write_volatile(page_start.add(offset), 0);
                }

                println!("[VirtIOHal] Page {} cleared successfully", page_idx);
            }
            println!("[VirtIOHal] All pages cleared successfully");
        }
        println!(
            "[VirtIOHal] Successfully zeroed {} pages of DMA memory",
            pages
        );

        // 将帧范围存储到全局映射表中
        DMA_ALLOCATIONS.lock().insert(phys_addr, frame_range);
        println!(
            "[VirtIOHal] DMA allocation complete: PA=0x{:x}, VA=0x{:x}, {} pages",
            phys_addr, virt_addr as usize, pages
        );

        (phys_addr, virt_ptr)
    }

    /// 释放之前分配的DMA内存
    unsafe fn dma_dealloc(paddr: PhysAddr, _vaddr: NonNull<u8>, _pages: usize) -> i32 {
        // 从全局映射表中查找并移除对应的帧范围
        // 注意：必须先释放DMA_ALLOCATIONS锁，再drop FrameRangeTracker
        // 因为FrameRangeTracker::drop()会获取FRAME_ALLOCATOR锁
        // 锁顺序要求：FRAME_ALLOCATOR(层级0) 必须在 DMA_ALLOCATIONS(层级7) 之前
        let frame_range = DMA_ALLOCATIONS.lock().remove(&paddr);
        // DMA_ALLOCATIONS锁已释放

        // 现在可以安全地drop frame_range，它会获取FRAME_ALLOCATOR锁
        if frame_range.is_some() {
            0 // 成功释放
        } else {
            -1 // 未找到对应的分配记录
        }
    }

    /// 将MMIO物理地址转换为虚拟地址
    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, size: usize) -> NonNull<u8> {
        // 提取物理地址值并使用架构特定的转换函数
        let phys_addr = paddr as usize;
        let virt = paddr_to_vaddr(phys_addr);

        println!(
            "[VirtIOHal] mmio_phys_to_virt: PA=0x{:x} -> VA=0x{:x}, size=0x{:x}",
            phys_addr, virt, size
        );

        // 验证虚拟地址的合法性
        let ptr = NonNull::new(virt as *mut u8).expect("mmio_phys_to_virt returned null pointer");

        println!("[VirtIOHal] mmio_phys_to_virt: returning valid pointer");
        ptr
    }

    /// 共享内存区域给设备，并返回设备可访问的物理地址
    unsafe fn share(buffer: NonNull<[u8]>, direction: BufferDirection) -> PhysAddr {
        let vaddr = buffer.as_ptr() as *const u8 as usize;
        let paddr = unsafe { vaddr_to_paddr(vaddr) };

        println!(
            "[VirtIOHal] share: VA=0x{:x} -> PA=0x{:x}, len={}, direction={:?}",
            vaddr,
            paddr,
            buffer.len(),
            direction
        );

        let result = PhysAddr::from(paddr as u64);
        println!("[VirtIOHal] share: returning PA=0x{:x}", result);
        result
    }

    /// 取消共享内存区域，并在必要时将数据复制回原始缓冲区
    unsafe fn unshare(paddr: PhysAddr, buffer: NonNull<[u8]>, direction: BufferDirection) {
        println!(
            "[VirtIOHal] unshare: PA=0x{:x}, len={}, direction={:?}",
            paddr,
            buffer.len(),
            direction
        );
        // 简化实现，由于使用直接映射，不需要额外操作
    }
}

impl VirtIOHal {
    /// 创建新的 HAL 实例
    pub fn new() -> Self {
        Self
    }
}
