use alloc::collections::btree_map::BTreeMap;
use core::cmp::min;

use crate::arch::mm::{paddr_to_vaddr, vaddr_to_paddr};
use crate::mm::address::{Paddr, PageNum, Ppn, UsizeConvert, Vpn, VpnRange};
use crate::mm::frame_allocator::{TrackedFrames, alloc_frame};
use crate::mm::page_table::{
    self, ActivePageTableInner, PageSize, PageTableInner, UniversalPTEFlag,
};

/// 映射策略类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapType {
    /// 直接映射（虚拟地址 = 物理地址 + VIRTUAL_BASE）
    Direct,
    /// 帧映射（从帧分配器分配）
    Framed,
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
}

/// 内存空间中的一个内存映射区域
#[derive(Debug)]
pub struct MappingArea {
    /// 此映射区域的虚拟页号范围
    ///
    /// 注意！
    ///
    /// 创建后不要更改它，
    /// 不要用它来映射或解除映射页
    vpn_range: VpnRange,

    /// 此映射区域的类型
    area_type: AreaType,

    /// 映射策略类型
    map_type: MapType,

    /// 此映射区域的权限（使用 UniversalPTEFlag 以提高性能）
    permission: UniversalPTEFlag,

    /// 用于帧映射区域的跟踪帧
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

    /// 获取虚拟页号（VPN）对应的物理页号（PPN）（如果已映射）
    pub fn get_ppn(&self, vpn: Vpn) -> Option<crate::mm::address::Ppn> {
        self.frames.get(&vpn).map(|tracked| match tracked {
            TrackedFrames::Single(frame) => frame.ppn(),
            TrackedFrames::Multiple(frames) => frames.first().map(|f| f.ppn()).unwrap(),
            TrackedFrames::Contiguous(_) => {
                // 当前简化的 4K-only 实现不支持连续帧
                panic!("当前实现不支持连续帧");
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

    /// 映射单个虚拟页到物理页
    pub fn map_one(
        &mut self,
        page_table: &mut ActivePageTableInner,
        vpn: Vpn,
    ) -> Result<(), page_table::PagingError> {
        let ppn = match self.map_type {
            MapType::Direct => {
                // 对于直接映射，VPN 等于 PPN + 偏移量
                let vaddr = vpn.start_addr();
                let paddr = unsafe { vaddr_to_paddr(vaddr.as_usize()) };
                Ppn::from_addr_floor(Paddr::from_usize(paddr))
            }
            MapType::Framed => {
                // 分配一个新的帧
                let frame = alloc_frame().ok_or(page_table::PagingError::FrameAllocFailed)?;
                let ppn = frame.ppn();
                self.frames.insert(vpn, TrackedFrames::Single(frame));
                ppn
            }
        };

        page_table.map(vpn, ppn, PageSize::Size4K, self.permission)?;
        Ok(())
    }

    /// 映射此映射区域中的所有页
    pub fn map(
        &mut self,
        page_table: &mut ActivePageTableInner,
    ) -> Result<(), page_table::PagingError> {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn)?;
        }
        Ok(())
    }

    /// 解除映射单个虚拟页
    pub fn unmap_one(
        &mut self,
        page_table: &mut ActivePageTableInner,
        vpn: Vpn,
    ) -> Result<(), page_table::PagingError> {
        page_table.unmap(vpn)?;

        // 对于帧映射，移除帧跟踪器
        if self.map_type == MapType::Framed {
            self.frames.remove(&vpn);
        }
        Ok(())
    }

    /// 解除映射此映射区域中的所有页
    pub fn unmap(
        &mut self,
        page_table: &mut ActivePageTableInner,
    ) -> Result<(), page_table::PagingError> {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn)?;
        }
        Ok(())
    }

    /// 复制数据到已映射的区域
    pub fn copy_data(&self, page_table: &mut ActivePageTableInner, data: &[u8], offset: usize) {
        let mut copied = 0;
        let total_len = data.len();

        for (i, vpn) in self.vpn_range.iter().enumerate() {
            if copied >= total_len {
                break;
            }

            // 获取此 VPN 对应的物理地址
            let vaddr = vpn.start_addr();
            let paddr = page_table.translate(vaddr).expect("无法转换虚拟地址");
            let paddr = if i == 0 {
                paddr.as_usize().checked_add(offset).unwrap()
            } else {
                paddr.as_usize()
            };

            // 计算此页需要复制多少数据
            let remaining = total_len - copied;
            let to_copy = min(remaining, crate::config::PAGE_SIZE);

            // 复制数据到物理页
            unsafe {
                let dst_va = paddr_to_vaddr(paddr);
                let dst = dst_va as *mut u8;
                let src = data.as_ptr().add(copied);
                core::ptr::copy_nonoverlapping(src, dst, to_copy);
            }

            copied += to_copy;
        }
    }

    /// 克隆元数据，但不克隆帧
    /// 用于写时复制（COW）的 fork
    pub fn clone_metadata(&self) -> Self {
        MappingArea {
            vpn_range: self.vpn_range,
            area_type: self.area_type,
            map_type: self.map_type,
            permission: self.permission,
            frames: BTreeMap::new(), // 不克隆帧
        }
    }

    /// 克隆映射区域及其数据
    /// 仅支持帧映射区域
    pub fn clone_with_data(
        &self,
        page_table: &mut ActivePageTableInner,
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

                    // 复制数据到新帧
                    unsafe {
                        let src_va = paddr_to_vaddr(src_ppn.start_addr().as_usize());
                        let dst_va = paddr_to_vaddr(new_ppn.start_addr().as_usize());

                        core::ptr::copy_nonoverlapping(
                            src_va as *const u8,
                            dst_va as *mut u8,
                            crate::config::PAGE_SIZE,
                        );
                    }

                    // 建立页表映射
                    page_table.map(*vpn, new_ppn, PageSize::Size4K, self.permission)?;

                    new_area
                        .frames
                        .insert(*vpn, TrackedFrames::Single(new_frame));
                }
                TrackedFrames::Multiple(frames) => {
                    // 复制多个不连续的页
                    let mut new_frames = alloc::vec::Vec::new();

                    for frame in frames.iter() {
                        let new_frame =
                            alloc_frame().ok_or(page_table::PagingError::FrameAllocFailed)?;
                        let new_ppn = new_frame.ppn();
                        let src_ppn = frame.ppn();

                        // 复制数据到新帧
                        unsafe {
                            let src_va = paddr_to_vaddr(src_ppn.start_addr().as_usize());
                            let dst_va = paddr_to_vaddr(new_ppn.start_addr().as_usize());

                            core::ptr::copy_nonoverlapping(
                                src_va as *const u8,
                                dst_va as *mut u8,
                                crate::config::PAGE_SIZE,
                            );
                        }

                        // 建立页表映射
                        page_table.map(*vpn, new_ppn, PageSize::Size4K, self.permission)?;

                        new_frames.push(new_frame);
                    }

                    new_area
                        .frames
                        .insert(*vpn, TrackedFrames::Multiple(new_frames));
                }
                // TODO(暂时注释): 大页克隆逻辑
                //
                // TrackedFrames::Contiguous(frame_range) => {
                //    // 复制连续页（大页）
                //    let num_pages = frame_range.len();
                //    let new_frame_range = crate::mm::frame_allocator::alloc_contig_frames_aligned(
                //        num_pages,
                //        num_pages,
                //    ).ok_or(page_table::PagingError::FrameAllocFailed)?;
                //
                //    let src_ppn = frame_range.start_ppn();
                //    let new_ppn = new_frame_range.start_ppn();
                //    let total_size = num_pages * crate::config::PAGE_SIZE;
                //
                //    // 复制数据到新帧
                //    unsafe {
                //        let src_va = paddr_to_vaddr(src_ppn.start_addr().as_usize());
                //        let dst_va = paddr_to_vaddr(new_ppn.start_addr().as_usize());
                //
                //        core::ptr::copy_nonoverlapping(
                //            src_va as *const u8,
                //            dst_va as *mut u8,
                //            total_size
                //        );
                //    }
                //
                //    // 建立页表映射 (根据页大小确定 PageSize)
                //    let page_size = match num_pages {
                //        262144 => PageSize::Size1G,  // 1GB = 262144 * 4KB
                //        512 => PageSize::Size2M,     // 2MB = 512 * 4KB
                //        _ => PageSize::Size4K,       // 其他情况使用 4K
                //    };
                //    page_table.map(*vpn, new_ppn, page_size, self.permission)?;
                //
                //    new_area.frames.insert(*vpn, TrackedFrames::Contiguous(new_frame_range));
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
// /// 大页映射实现
// ///
// /// 推荐使用场景：
// /// * 映射内核物理内存区域
// /// * 映射内核代码和只读数据段
// /// * Mmap 大型文件
// /// * 进程间的共享内存区域
// /// * 不经常分配和释放的大内存区域
// ///
// /// 不推荐使用场景：
// /// * 在用户空间中使用
// /// * 经常分配和释放的小内存区域
// ///
// /// TODO: 实现将大页拆分为小页的功能（如果需要，例如用于 COW）
// impl MappingArea {
//     /// 使用贪心算法映射为大页
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

/// 动态扩展和收缩
impl MappingArea {
    // TODO(暂时注释): 支持大页的扩展方法
    //
    // /// 通过在末尾添加页来扩展区域
    // ///
    // /// 如果对齐和大小允许，支持大页分配
    // /// 返回新的结束 VPN
    // pub fn extend(
    //    &mut self,
    //    page_table: &mut ActivePageTableInner,
    //    count: usize,
    // ) -> Result<Vpn, page_table::PagingError> {
    //    let old_end = self.vpn_range.end();
    //    let new_end = Vpn::from_usize(old_end.as_usize() + count);
    //
    //    let start_va = old_end.start_addr();
    //    let end_va = new_end.start_addr();
    //    let mut current_va = start_va;
    //
    //    // 使用贪心算法映射新页（支持大页）
    //    while current_va < end_va {
    //        let remaining = end_va.as_usize() - current_va.as_usize();
    //        let current_vpn = Vpn::from_addr_floor(current_va);
    //
    //        // 尝试 1GB 页
    //        if remaining >= PageSize::Size1G as usize
    //            && current_va.as_usize() % (PageSize::Size1G as usize) == 0
    //        {
    //            let ppn = self.allocate_for_huge_page(current_vpn, 262144)?;
    //            page_table.map(current_vpn, ppn, PageSize::Size1G, self.permission)?;
    //            current_va = current_va + PageSize::Size1G as usize;
    //        }
    //        // 尝试 2MB 页
    //        else if remaining >= PageSize::Size2M as usize
    //            && current_va.as_usize() % (PageSize::Size2M as usize) == 0
    //        {
    //            let ppn = self.allocate_for_huge_page(current_vpn, 512)?;
    //            page_table.map(current_vpn, ppn, PageSize::Size2M, self.permission)?;
    //            current_va = current_va + PageSize::Size2M as usize;
    //        }
    //        // 使用 4KB 页
    //        else {
    //            let ppn = self.allocate_for_small_page(current_vpn)?;
    //            page_table.map(current_vpn, ppn, PageSize::Size4K, self.permission)?;
    //            current_va = current_va + PageSize::Size4K as usize;
    //        }
    //    }
    //
    //    // 更新范围
    //    self.vpn_range = VpnRange::new(self.vpn_range.start(), new_end);
    //
    //    Ok(new_end)
    // }

    /// 通过在末尾添加页来扩展区域（仅限 4K 页）
    ///
    /// 返回新的结束 VPN
    pub fn extend(
        &mut self,
        page_table: &mut ActivePageTableInner,
        count: usize,
    ) -> Result<Vpn, page_table::PagingError> {
        let old_end = self.vpn_range.end();
        let new_end = Vpn::from_usize(old_end.as_usize() + count);

        // 仅使用 4K 页映射每个新页
        for i in 0..count {
            let vpn = Vpn::from_usize(old_end.as_usize() + i);
            self.map_one(page_table, vpn)?;
        }

        // 更新范围
        self.vpn_range = VpnRange::new(self.vpn_range.start(), new_end);

        Ok(new_end)
    }

    // TODO(暂时注释): 支持大页的收缩方法
    //
    // /// 通过从末尾移除页来收缩区域
    // ///
    // /// 处理大页边界：
    // /// - 如果移除整个大页：直接解除映射
    // /// - 如果部分移除一个大页：当前返回错误
    // ///  (TODO: 如果需要，实现大页拆分)
    // ///
    // /// 返回新的结束 VPN
    // pub fn shrink(
    //    &mut self,
    //    page_table: &mut ActivePageTableInner,
    //    count: usize,
    // ) -> Result<Vpn, page_table::PagingError> {
    //    if count > self.vpn_range.len() {
    //        return Err(page_table::PagingError::ShrinkBelowStart);
    //    }
    //
    //    let old_end = self.vpn_range.end();
    //    let new_end = Vpn::from_usize(old_end.as_usize() - count);
    //
    //    // 收集需要移除的 VPN 范围
    //    let remove_range = VpnRange::new(new_end, old_end);
    //
    //    // 检查并移除帧
    //    // 需要从后向前遍历，以便正确处理大页
    //    let mut vpns_to_remove: alloc::vec::Vec<Vpn> = remove_range.into_iter().collect();
    //    vpns_to_remove.sort_by(|a, b| b.cmp(a)); // 降序排列
    //
    //    let mut i = 0;
    //    while i < vpns_to_remove.len() {
    //        let vpn = vpns_to_remove[i];
    //
    //        // 检查这个 VPN 是否有对应的帧记录
    //        if let Some(tracked_frames) = self.frames.get(&vpn) {
    //            match tracked_frames {
    //                TrackedFrames::Single(_) => {
    //                    // 单个 4K 页，直接取消映射
    //                    page_table.unmap(vpn)?;
    //                    self.frames.remove(&vpn);
    //                    i += 1;
    //                }
    //                TrackedFrames::Multiple(_) => {
    //                    // 多个不连续页，直接取消映射
    //                    page_table.unmap(vpn)?;
    //                    self.frames.remove(&vpn);
    //                    i += 1;
    //                }
    //                TrackedFrames::Contiguous(frame_range) => {
    //                    // 连续页（大页）
    //                    let num_pages = frame_range.len();
    //
    //                    // 检查是否要移除整个大页
    //                    let huge_page_vpns: alloc::vec::Vec<Vpn> = (vpn.as_usize()..vpn.as_usize() + num_pages)
    //                        .map(|v| Vpn::from_usize(v))
    //                        .collect();
    //
    //                    let all_in_remove = huge_page_vpns.iter()
    //                        .all(|v| remove_range.contains(*v));
    //
    //                    if all_in_remove {
    //                        // 整个大页都要移除，直接取消映射
    //                        page_table.unmap(vpn)?;
    //                        self.frames.remove(&vpn);
    //                        i += num_pages;
    //                    } else {
    //                        // 部分移除大页：目前不支持，需要实现大页拆分
    //                        return Err(page_table::PagingError::HugePageSplitNotImplemented);
    //                    }
    //                }
    //            }
    //        } else {
    //            // 没有帧记录（可能是恒等映射），只需取消映射
    //            page_table.unmap(vpn)?;
    //            i += 1;
    //        }
    //    }
    //
    //    // 更新范围
    //    self.vpn_range = VpnRange::new(self.vpn_range.start(), new_end);
    //
    //    Ok(new_end)
    // }

    /// 通过从末尾移除页来收缩区域（仅限 4K 页）
    ///
    /// 返回新的结束 VPN
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

        // 从末尾解除映射页（倒序）
        for i in (0..count).rev() {
            let vpn = Vpn::from_usize(new_end.as_usize() + i);
            self.unmap_one(page_table, vpn)?;
        }

        // 更新范围
        self.vpn_range = VpnRange::new(self.vpn_range.start(), new_end);

        Ok(new_end)
    }
}
