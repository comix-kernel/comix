//! HAL (硬件抽象层) 实现，用于适配 virtio-drivers 0.12.0 库
use alloc::collections::btree_map::BTreeMap;
use core::ptr::NonNull;
use lazy_static::lazy_static;
use virtio_drivers::{BufferDirection, Hal, PhysAddr};
use crate::mm::address::{ConvertablePaddr, PageNum, UsizeConvert};
use crate::mm::frame_allocator::FrameRangeTracker;
use crate::sync::SpinLock;

// 全局映射表，用于跟踪物理地址到分配的帧范围的映射
lazy_static! {
    static ref DMA_ALLOCATIONS: SpinLock<BTreeMap<PhysAddr, FrameRangeTracker>> = 
        SpinLock::new(BTreeMap::new());
}

/// virtio-drivers 0.12.0 库使用的 HAL 实现
pub struct VirtIOHal;

unsafe impl Hal for VirtIOHal {
    /// 分配并清零指定数量的连续物理页用于DMA
    fn dma_alloc(
        pages: usize,
        _direction: BufferDirection,
    ) -> (PhysAddr, NonNull<u8>) {
        // 使用系统的连续物理帧分配器
        let frame_range = crate::mm::frame_allocator::alloc_contig_frames(pages)
            .expect("Failed to allocate contiguous frames for DMA");
        
        // 获取起始物理页号
        let start_ppn = frame_range.start_ppn();
        
        // 计算物理地址
        let phys_addr = PhysAddr::from(start_ppn.start_addr().as_usize() as u64);
        
        // 在RISC-V架构中，内核空间使用直接映射
        // 将物理地址转换为虚拟地址
        let virt_addr = unsafe { start_ppn.start_addr().to_vaddr().as_mut_ptr::<u8>() };
        let virt_ptr = NonNull::new(virt_addr).unwrap();
        
        // 将帧范围存储到全局映射表中
        DMA_ALLOCATIONS.lock().insert(phys_addr, frame_range);
        
        (phys_addr, virt_ptr)
    }

    /// 释放之前分配的DMA内存
    unsafe fn dma_dealloc(
        paddr: PhysAddr,
        _vaddr: NonNull<u8>,
        _pages: usize,
    ) -> i32 {
        // 从全局映射表中查找并移除对应的帧范围
        // 当frame_range被drop时，它会自动释放分配的物理帧
        if DMA_ALLOCATIONS.lock().remove(&paddr).is_some() {
            0 // 成功释放
        } else {
            -1 // 未找到对应的分配记录
        }
    }
    
    /// 将MMIO物理地址转换为虚拟地址
    unsafe fn mmio_phys_to_virt(
        paddr: PhysAddr,
        _size: usize,
    ) -> NonNull<u8> {
        // 在RISC-V架构中，物理地址映射到虚拟地址通常是通过添加VADDR_START偏移
        // 这里简化实现，直接使用物理地址作为虚拟地址
        let vaddr = paddr as *mut u8;
        NonNull::new(vaddr).unwrap()
    }
    
    /// 共享内存区域给设备，并返回设备可访问的物理地址
    unsafe fn share(
        buffer: NonNull<[u8]>,
        _direction: BufferDirection,
    ) -> PhysAddr {
        let vaddr = buffer.as_ptr() as *const u8 as usize;
        PhysAddr::from(vaddr as u64)
    }
    
    /// 取消共享内存区域，并在必要时将数据复制回原始缓冲区
    unsafe fn unshare(
        _paddr: PhysAddr,
        _buffer: NonNull<[u8]>,
        _direction: BufferDirection,
    ) {
        // 简化实现，由于使用直接映射，不需要额外操作
    }
}

impl VirtIOHal {
    /// 创建新的 HAL 实例
    pub fn new() -> Self {
        Self
    }
}