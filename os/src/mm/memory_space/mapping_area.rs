use alloc::collections::btree_map::BTreeMap;
use core::cmp::min;

use crate::arch::mm::{paddr_to_vaddr, vaddr_to_paddr};
use crate::mm::address::{ConvertablePaddr, Paddr, PageNum, Ppn, UsizeConvert, Vpn, VpnRange};
use crate::mm::frame_allocator::{TrackedFrames, alloc_frame};
use crate::mm::page_table::{
    self, ActivePageTableInner, PageSize, PageTableInner, UniversalPTEFlag,
};

/// Mapping stretagy type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapType {
    /// Direct mapped (virtual address = physical address + VIRTUAL_BASE)
    Direct,
    /// Frame mapped (allocated from frame allocator)
    Framed,
}

/// Type of the memory area
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaType {
    KernelText,   // Kernel code segment
    KernelRodata, // Kernel read-only data segment
    KernelData,   // Kernel data segment
    KernelStack,  // Kernel stack
    KernelBss,    // Kernel BSS segment
    KernelHeap,   // Kernel heap
    KernelMmio,   // Kernel memory-mapped I/O
    UserText,     // User code segment
    UserRodata,   // User read-only data segment
    UserData,     // User data segment
    UserBss,      // User BSS segment
    UserStack,    // User stack
    UserHeap,     // User heap
}

/// A memory mapping area in a memory space
#[derive(Debug)]
pub struct MappingArea {
    /// Virtual page number range of this mapping area
    ///
    /// Attention!
    ///
    /// don't change it after created,
    /// don't use it to map or unmap pages
    vpn_range: VpnRange,

    /// Type of this mapping area
    area_type: AreaType,

    /// Mapping strategy type
    map_type: MapType,

    /// Permission permission for this mapping area (use UniversalPTEFlag for perfermance)
    permission: UniversalPTEFlag,

    /// Tracked frames for framed mapping area
    frames: BTreeMap<Vpn, TrackedFrames>,
}

impl MappingArea {
    pub fn vpn_range(&self) -> VpnRange {
        self.vpn_range
    }

    pub fn permission(&self) -> UniversalPTEFlag {
        self.permission
    }

    pub fn map_type(&self) -> MapType {
        self.map_type
    }

    pub fn area_type(&self) -> AreaType {
        self.area_type
    }

    /// Gets the PPN for a VPN (if mapped)
    pub fn get_ppn(&self, vpn: Vpn) -> Option<crate::mm::address::Ppn> {
        self.frames.get(&vpn).map(|tracked| match tracked {
            TrackedFrames::Single(frame) => frame.ppn(),
            TrackedFrames::Multiple(frames) => frames.first().map(|f| f.ppn()).unwrap(),
            TrackedFrames::Contiguous(_) => {
                // Not supported for simplified 4K-only implementation
                panic!("Contiguous frames not supported in current implementation");
            }
        })
    }

    pub fn new(
        vpn_range: VpnRange,
        area_type: AreaType,
        map_type: MapType,
        permission: UniversalPTEFlag,
    ) -> Self {
        MappingArea {
            vpn_range,
            area_type,
            map_type,
            permission,
            frames: BTreeMap::new(),
        }
    }

    /// Map a single virtual page to a physical page
    pub fn map_one(
        &mut self,
        page_table: &mut ActivePageTableInner,
        vpn: Vpn,
    ) -> Result<(), page_table::PagingError> {
        let ppn = match self.map_type {
            MapType::Direct => {
                // For direct mapping, vpn equals ppn + offset
                let vaddr = vpn.start_addr();
                let paddr = vaddr_to_paddr(vaddr.as_usize());
                Ppn::from_addr_floor(Paddr::from_usize(paddr))
            }
            MapType::Framed => {
                // Allocate a new frame
                let frame = alloc_frame().ok_or(page_table::PagingError::FrameAllocFailed)?;
                let ppn = frame.ppn();
                self.frames.insert(vpn, TrackedFrames::Single(frame));
                ppn
            }
        };

        page_table.map(vpn, ppn, PageSize::Size4K, self.permission)?;
        Ok(())
    }

    /// Map all pages in this mapping area
    pub fn map(
        &mut self,
        page_table: &mut ActivePageTableInner,
    ) -> Result<(), page_table::PagingError> {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn)?;
        }
        Ok(())
    }

    /// Unmap a single virtual page
    pub fn unmap_one(
        &mut self,
        page_table: &mut ActivePageTableInner,
        vpn: Vpn,
    ) -> Result<(), page_table::PagingError> {
        page_table.unmap(vpn)?;

        // For framed mapping, remove the frame tracker
        if self.map_type == MapType::Framed {
            self.frames.remove(&vpn);
        }
        Ok(())
    }

    /// Unmap all pages in this mapping area
    pub fn unmap(
        &mut self,
        page_table: &mut ActivePageTableInner,
    ) -> Result<(), page_table::PagingError> {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn)?;
        }
        Ok(())
    }

    /// Copy data to the mapped area
    pub fn copy_data(&self, page_table: &mut ActivePageTableInner, data: &[u8]) {
        let mut copied = 0;
        let total_len = data.len();

        for vpn in self.vpn_range {
            if copied >= total_len {
                break;
            }

            // Get the physical address for this vpn
            let vaddr = vpn.start_addr();
            let paddr = page_table
                .translate(vaddr)
                .expect("failed to translate virtual address");

            // Calculate how much to copy for this page
            let remaining = total_len - copied;
            let to_copy = min(remaining, crate::config::PAGE_SIZE);

            // Copy data to the physical page
            unsafe {
                let dst_va = paddr_to_vaddr(paddr.as_usize());
                let dst = dst_va as *mut u8;
                let src = data.as_ptr().add(copied);
                core::ptr::copy_nonoverlapping(src, dst, to_copy);
            }

            copied += to_copy;
        }
    }

    /// Clone metadata without cloning the frames
    /// Used for Copy-on-Write (COW) fork
    pub fn clone_metadata(&self) -> Self {
        MappingArea {
            vpn_range: self.vpn_range,
            area_type: self.area_type,
            map_type: self.map_type,
            permission: self.permission,
            frames: BTreeMap::new(), // Don't clone frames
        }
    }

    /// Clone the mapping area along with its data
    /// Only supports framed mapping areas
    pub fn clone_with_data(
        &self,
        _page_table: &mut ActivePageTableInner,
    ) -> Result<Self, page_table::PagingError> {
        let mut new_area = self.clone_metadata();
        if self.map_type != MapType::Framed {
            return Err(page_table::PagingError::UnsupportedMapType);
        }

        // 遍历原 area 的 frames BTreeMap
        for (vpn, tracked_frames) in &self.frames {
            match tracked_frames {
                TrackedFrames::Single(frame) => {
                    // 复制单个 4K 页
                    let new_frame =
                        alloc_frame().ok_or(page_table::PagingError::FrameAllocFailed)?;
                    let new_ppn = new_frame.ppn();
                    let src_ppn = frame.ppn();

                    unsafe {
                        let src_va = paddr_to_vaddr(src_ppn.start_addr().as_usize());
                        let dst_va = paddr_to_vaddr(new_ppn.start_addr().as_usize());

                        core::ptr::copy_nonoverlapping(
                            src_va as *const u8,
                            dst_va as *mut u8,
                            crate::config::PAGE_SIZE,
                        );
                    }

                    new_area
                        .frames
                        .insert(*vpn, TrackedFrames::Single(new_frame));
                }
                TrackedFrames::Multiple(frames) => {
                    // 复制多个不连续的页
                    let mut new_frames = alloc::vec::Vec::new();

                    for frame in frames {
                        let new_frame =
                            alloc_frame().ok_or(page_table::PagingError::FrameAllocFailed)?;
                        let new_ppn = new_frame.ppn();
                        let src_ppn = frame.ppn();

                        unsafe {
                            let src_va = paddr_to_vaddr(src_ppn.start_addr().as_usize());
                            let dst_va = paddr_to_vaddr(new_ppn.start_addr().as_usize());

                            core::ptr::copy_nonoverlapping(
                                src_va as *const u8,
                                dst_va as *mut u8,
                                crate::config::PAGE_SIZE,
                            );
                        }

                        new_frames.push(new_frame);
                    }

                    new_area
                        .frames
                        .insert(*vpn, TrackedFrames::Multiple(new_frames));
                }
                // TODO(暂时注释): 大页克隆逻辑
                //
                // TrackedFrames::Contiguous(frame_range) => {
                //     // 复制连续页（大页）
                //     let num_pages = frame_range.len();
                //     let new_frame_range = crate::mm::frame_allocator::alloc_contig_frames_aligned(
                //         num_pages,
                //         num_pages,
                //     ).ok_or(page_table::PagingError::FrameAllocFailed)?;
                //
                //     let src_ppn = frame_range.start_ppn();
                //     let new_ppn = new_frame_range.start_ppn();
                //     let total_size = num_pages * crate::config::PAGE_SIZE;
                //
                //     unsafe {
                //         let src_va = paddr_to_vaddr(src_ppn.start_addr().as_usize());
                //         let dst_va = paddr_to_vaddr(new_ppn.start_addr().as_usize());
                //
                //         core::ptr::copy_nonoverlapping(
                //             src_va as *const u8,
                //             dst_va as *mut u8,
                //             total_size
                //         );
                //     }
                //
                //     new_area.frames.insert(*vpn, TrackedFrames::Contiguous(new_frame_range));
                // }
                TrackedFrames::Contiguous(_) => {
                    // 当前不支持大页克隆（已暂时禁用大页功能）
                    return Err(page_table::PagingError::HugePageSplitNotImplemented);
                }
            }
        }

        Ok(new_area)
    }
}

// TODO(暂时注释): 大页映射实现，包含贪心算法选择1GB/2MB/4K页
//
// /// Huge Page Mapping Implementation
// ///
// /// Recommended scenarios for use:
// /// * Mapping kernel physical memory regions
// /// * Mapping kernel text and rodata segments
// /// * Mmap large files
// /// * Shared memory regions between processes
// /// * Large memory rigions that are infrequently allocated and deallocated
// ///
// /// Not recommended scenarios for use:
// /// * Use it in User Space
// /// * Small memory regions that are frequently allocated and deallocated
// ///
// /// TODO: Implement split huge page to small pages (if needed, e.g., for COW)
// impl MappingArea {
//     /// Map to Huge Pages greedily
//     pub fn map_with_huge_page(&mut self, page_table: &mut ActivePageTableInner) -> Result<(), page_table::PagingError> {
//         let start_va = self.vpn_range.start().start_addr();
//         let end_va = self.vpn_range.end().start_addr();
//         let mut current_va = start_va;
//
//         while current_va < end_va {
//             let remaining = end_va.as_usize() - current_va.as_usize();
//             let current_vpn = Vpn::from_addr_floor(current_va);
//
//             // 贪心算法：优先尝试大页
//
//             // 尝试 1GB 页
//             if remaining >= PageSize::Size1G as usize
//                 && current_va.as_usize() % (PageSize::Size1G as usize) == 0
//             {
//                 let ppn = self.allocate_for_huge_page(
//                     current_vpn,
//                     262144,  // 1GB = 262144 pages
//                 )?;
//
//                 page_table.map(
//                     current_vpn,
//                     ppn,
//                     PageSize::Size1G,
//                     self.permission,
//                 )?;
//
//                 current_va = current_va + PageSize::Size1G as usize;
//             }
//             // 尝试 2MB 页
//             else if remaining >= PageSize::Size2M as usize
//                 && current_va.as_usize() % (PageSize::Size2M as usize) == 0
//             {
//                 let ppn = self.allocate_for_huge_page(
//                     current_vpn,
//                     512,  // 2MB = 512 pages
//                 )?;
//
//                 page_table.map(
//                     current_vpn,
//                     ppn,
//                     PageSize::Size2M,
//                     self.permission,
//                 )?;
//
//                 current_va = current_va + PageSize::Size2M as usize;
//             }
//             // 使用 4KB 页
//             else {
//                 let ppn = self.allocate_for_small_page(current_vpn)?;
//
//                 page_table.map(
//                     current_vpn,
//                     ppn,
//                     PageSize::Size4K,
//                     self.permission,
//                 )?;
//
//                 current_va = current_va + PageSize::Size4K as usize;
//             }
//         }
//
//         Ok(())
//     }
//
//     fn allocate_for_huge_page(&mut self, vpn: Vpn, num_pages: usize) -> Result<Ppn, page_table::PagingError> {
//         match self.map_type {
//             MapType::Direct => {
//                 // 恒等映射：PPN = VPN
//                 Ok(Ppn::from_usize(vpn.as_usize()))
//             }
//             MapType::Framed => {
//                 // 使用对齐分配
//                 let frame_range = crate::mm::frame_allocator::alloc_contig_frames_aligned(
//                     num_pages,
//                     num_pages,  // 对齐要求 = 页数
//                 ).ok_or(page_table::PagingError::InvalidAddress)?;
//
//                 let ppn = frame_range.start_ppn();
//
//                 // 存储到 frames（使用 Contiguous 变体）
//                 self.frames.insert(vpn, TrackedFrames::Contiguous(frame_range));
//
//                 Ok(ppn)
//             }
//         }
//     }
//
//     fn allocate_for_small_page(&mut self, vpn: Vpn) -> Result<Ppn, page_table::PagingError> {
//         match self.map_type {
//             MapType::Direct => {
//                 Ok(Ppn::from_usize(vpn.as_usize()))
//             }
//             MapType::Framed => {
//                 let frame = crate::mm::frame_allocator::alloc_frame()
//                     .ok_or(page_table::PagingError::InvalidAddress)?;
//                 let ppn = frame.ppn();
//                 self.frames.insert(vpn, TrackedFrames::Single(frame));
//                 Ok(ppn)
//             }
//         }
//     }
// }

/// Dynamic Extension and Shrinking
impl MappingArea {
    // TODO(暂时注释): 支持大页的扩展方法
    //
    // /// Extends the area by adding pages at the end
    // ///
    // /// Supports huge page allocation if alignment and size permit
    // /// Returns the new end VPN
    // pub fn extend(
    //     &mut self,
    //     page_table: &mut ActivePageTableInner,
    //     count: usize,
    // ) -> Result<Vpn, page_table::PagingError> {
    //     let old_end = self.vpn_range.end();
    //     let new_end = Vpn::from_usize(old_end.as_usize() + count);
    //
    //     let start_va = old_end.start_addr();
    //     let end_va = new_end.start_addr();
    //     let mut current_va = start_va;
    //
    //     // 使用贪心算法映射新页（支持大页）
    //     while current_va < end_va {
    //         let remaining = end_va.as_usize() - current_va.as_usize();
    //         let current_vpn = Vpn::from_addr_floor(current_va);
    //
    //         // 尝试 1GB 页
    //         if remaining >= PageSize::Size1G as usize
    //             && current_va.as_usize() % (PageSize::Size1G as usize) == 0
    //         {
    //             let ppn = self.allocate_for_huge_page(current_vpn, 262144)?;
    //             page_table.map(current_vpn, ppn, PageSize::Size1G, self.permission)?;
    //             current_va = current_va + PageSize::Size1G as usize;
    //         }
    //         // 尝试 2MB 页
    //         else if remaining >= PageSize::Size2M as usize
    //             && current_va.as_usize() % (PageSize::Size2M as usize) == 0
    //         {
    //             let ppn = self.allocate_for_huge_page(current_vpn, 512)?;
    //             page_table.map(current_vpn, ppn, PageSize::Size2M, self.permission)?;
    //             current_va = current_va + PageSize::Size2M as usize;
    //         }
    //         // 使用 4KB 页
    //         else {
    //             let ppn = self.allocate_for_small_page(current_vpn)?;
    //             page_table.map(current_vpn, ppn, PageSize::Size4K, self.permission)?;
    //             current_va = current_va + PageSize::Size4K as usize;
    //         }
    //     }
    //
    //     // 更新范围
    //     self.vpn_range = VpnRange::new(self.vpn_range.start(), new_end);
    //
    //     Ok(new_end)
    // }

    /// Extends the area by adding pages at the end (4K pages only)
    ///
    /// Returns the new end VPN
    pub fn extend(
        &mut self,
        page_table: &mut ActivePageTableInner,
        count: usize,
    ) -> Result<Vpn, page_table::PagingError> {
        let old_end = self.vpn_range.end();
        let new_end = Vpn::from_usize(old_end.as_usize() + count);

        // Map each new page using 4K pages only
        for i in 0..count {
            let vpn = Vpn::from_usize(old_end.as_usize() + i);
            self.map_one(page_table, vpn)?;
        }

        // Update the range
        self.vpn_range = VpnRange::new(self.vpn_range.start(), new_end);

        Ok(new_end)
    }

    // TODO(暂时注释): 支持大页的收缩方法
    //
    // /// Shrinks the area by removing pages from the end
    // ///
    // /// Handles huge page boundaries:
    // /// - If removing entire huge pages: unmap them directly
    // /// - If partially removing a huge page: currently returns an error
    // ///   (TODO: implement huge page splitting if needed)
    // ///
    // /// Returns the new end VPN
    // pub fn shrink(
    //     &mut self,
    //     page_table: &mut ActivePageTableInner,
    //     count: usize,
    // ) -> Result<Vpn, page_table::PagingError> {
    //     if count > self.vpn_range.len() {
    //         return Err(page_table::PagingError::ShrinkBelowStart);
    //     }
    //
    //     let old_end = self.vpn_range.end();
    //     let new_end = Vpn::from_usize(old_end.as_usize() - count);
    //
    //     // 收集需要移除的 VPN 范围
    //     let remove_range = VpnRange::new(new_end, old_end);
    //
    //     // 检查并移除帧
    //     // 需要从后向前遍历，以便正确处理大页
    //     let mut vpns_to_remove: alloc::vec::Vec<Vpn> = remove_range.into_iter().collect();
    //     vpns_to_remove.sort_by(|a, b| b.cmp(a)); // 降序排列
    //
    //     let mut i = 0;
    //     while i < vpns_to_remove.len() {
    //         let vpn = vpns_to_remove[i];
    //
    //         // 检查这个 VPN 是否有对应的帧记录
    //         if let Some(tracked_frames) = self.frames.get(&vpn) {
    //             match tracked_frames {
    //                 TrackedFrames::Single(_) => {
    //                     // 单个 4K 页，直接取消映射
    //                     page_table.unmap(vpn)?;
    //                     self.frames.remove(&vpn);
    //                     i += 1;
    //                 }
    //                 TrackedFrames::Multiple(_) => {
    //                     // 多个不连续页，直接取消映射
    //                     page_table.unmap(vpn)?;
    //                     self.frames.remove(&vpn);
    //                     i += 1;
    //                 }
    //                 TrackedFrames::Contiguous(frame_range) => {
    //                     // 连续页（大页）
    //                     let num_pages = frame_range.len();
    //
    //                     // 检查是否要移除整个大页
    //                     let huge_page_vpns: alloc::vec::Vec<Vpn> = (vpn.as_usize()..vpn.as_usize() + num_pages)
    //                         .map(|v| Vpn::from_usize(v))
    //                         .collect();
    //
    //                     let all_in_remove = huge_page_vpns.iter()
    //                         .all(|v| remove_range.contains(*v));
    //
    //                     if all_in_remove {
    //                         // 整个大页都要移除，直接取消映射
    //                         page_table.unmap(vpn)?;
    //                         self.frames.remove(&vpn);
    //                         i += num_pages;
    //                     } else {
    //                         // 部分移除大页：目前不支持，需要实现大页拆分
    //                         return Err(page_table::PagingError::HugePageSplitNotImplemented);
    //                     }
    //                 }
    //             }
    //         } else {
    //             // 没有帧记录（可能是恒等映射），只需取消映射
    //             page_table.unmap(vpn)?;
    //             i += 1;
    //         }
    //     }
    //
    //     // 更新范围
    //     self.vpn_range = VpnRange::new(self.vpn_range.start(), new_end);
    //
    //     Ok(new_end)
    // }

    /// Shrinks the area by removing pages from the end (4K pages only)
    ///
    /// Returns the new end VPN
    pub fn shrink(
        &mut self,
        page_table: &mut ActivePageTableInner,
        count: usize,
    ) -> Result<Vpn, page_table::PagingError> {
        if count > self.vpn_range.len() {
            return Err(page_table::PagingError::ShrinkBelowStart);
        }

        let old_end = self.vpn_range.end();
        let new_end = Vpn::from_usize(old_end.as_usize() - count);

        // Unmap pages from the end (reverse order)
        for i in (0..count).rev() {
            let vpn = Vpn::from_usize(new_end.as_usize() + i);
            self.unmap_one(page_table, vpn)?;
        }

        // Update the range
        self.vpn_range = VpnRange::new(self.vpn_range.start(), new_end);

        Ok(new_end)
    }
}
