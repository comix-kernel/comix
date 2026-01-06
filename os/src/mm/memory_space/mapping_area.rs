use alloc::collections::btree_map::BTreeMap;
use core::cmp::min;

use crate::arch::mm::{TlbBatchContext, paddr_to_vaddr, vaddr_to_paddr};
use crate::config::PAGE_SIZE;
use crate::mm::address::{Paddr, PageNum, Ppn, UsizeConvert, Vpn, VpnRange};
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
    ///
    /// 现已移除大页映射，不用在意
    /// ~~ 注意！~~
    ///
    /// ~~ 创建后不要更改它，~~
    /// ~~ 不要用它来映射或解除映射页~~
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

impl MappingArea {
    pub fn vpn_range(&self) -> VpnRange {
        self.vpn_range
    }

    pub fn permission(&self) -> UniversalPTEFlag {
        self.permission.clone()
    }

    pub fn set_permission(&mut self, perm: UniversalPTEFlag) {
        self.permission = perm;
    }

    pub fn map_type(&self) -> MapType {
        self.map_type
    }

    pub fn area_type(&self) -> AreaType {
        self.area_type
    }

    /// 已实际映射的页数（仅对 Framed 有意义）
    ///
    /// 注意：Range/VPN/PPN 语义均为左闭右开。
    pub fn mapped_pages(&self) -> usize {
        match self.map_type {
            MapType::Framed => self
                .frames
                .values()
                .map(|t| match t {
                    TrackedFrames::Single(_) => 1,
                    TrackedFrames::Multiple(v) => v.len(),
                    TrackedFrames::Contiguous(r) => r.len(),
                })
                .sum(),
            _ => 0,
        }
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
        file: Option<MmapFile>,
    ) -> Self {
        MappingArea {
            vpn_range,
            area_type,
            map_type,
            permission,
            frames: BTreeMap::new(),
            file,
        }
    }

    /// 映射单个虚拟页到物理页
    pub fn map_one(
        &mut self,
        page_table: &mut ActivePageTableInner,
        vpn: Vpn,
    ) -> Result<(), page_table::PagingError> {
        self.map_one_with_batch(page_table, vpn, None)
    }

    /// 映射单个虚拟页到物理页（支持批处理）
    fn map_one_with_batch(
        &mut self,
        page_table: &mut ActivePageTableInner,
        vpn: Vpn,
        batch: Option<&mut TlbBatchContext>,
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
            MapType::Reserved => {
                // PROT_NONE：不建立页表映射
                return Ok(());
            }
        };

        page_table.map_with_batch(vpn, ppn, PageSize::Size4K, self.permission.clone(), batch)?;
        Ok(())
    }

    /// 映射此映射区域中的所有页
    pub fn map(
        &mut self,
        page_table: &mut ActivePageTableInner,
    ) -> Result<(), page_table::PagingError> {
        TlbBatchContext::execute(|batch| {
            for vpn in self.vpn_range {
                self.map_one_with_batch(page_table, vpn, Some(batch))?;
            }
            Ok(())
        })
    }

    /// 解除映射单个虚拟页
    pub fn unmap_one(
        &mut self,
        page_table: &mut ActivePageTableInner,
        vpn: Vpn,
    ) -> Result<(), page_table::PagingError> {
        self.unmap_one_with_batch(page_table, vpn, None)
    }

    /// 解除映射单个虚拟页（支持批处理）
    fn unmap_one_with_batch(
        &mut self,
        page_table: &mut ActivePageTableInner,
        vpn: Vpn,
        batch: Option<&mut TlbBatchContext>,
    ) -> Result<(), page_table::PagingError> {
        if self.map_type == MapType::Reserved {
            return Ok(());
        }
        page_table.unmap_with_batch(vpn, batch)?;

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
        TlbBatchContext::execute(|batch| {
            for vpn in self.vpn_range {
                self.unmap_one_with_batch(page_table, vpn, Some(batch))?;
            }
            Ok(())
        })
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

            let page_capacity = if i == 0 {
                crate::config::PAGE_SIZE - offset
            } else {
                crate::config::PAGE_SIZE
            };

            // 计算此页需要复制多少数据
            let remaining = total_len - copied;
            let to_copy = min(remaining, page_capacity);

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
            permission: self.permission.clone(),
            frames: BTreeMap::new(), // 不克隆帧
            // fork 时复制文件映射信息（MAP_SHARED 和 MAP_PRIVATE 都需要）
            file: self.file.as_ref().map(|f| MmapFile {
                file: f.file.clone(),
                offset: f.offset,
                len: f.len,
                prot: f.prot,
                flags: f.flags,
            }),
        }
    }

    /// 克隆映射区域及其数据
    /// 仅支持帧映射区域
    pub fn clone_with_data(
        &self,
        page_table: &mut ActivePageTableInner,
    ) -> Result<Self, page_table::PagingError> {
        use crate::arch::mm::TlbBatchContext;

        let mut new_area = self.clone_metadata();
        if self.map_type != MapType::Framed {
            return Err(page_table::PagingError::UnsupportedMapType);
        }

        TlbBatchContext::execute(|batch| {
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
                        page_table.map_with_batch(
                            *vpn,
                            new_ppn,
                            PageSize::Size4K,
                            self.permission.clone(),
                            Some(batch),
                        )?;

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
                            page_table.map_with_batch(
                                *vpn,
                                new_ppn,
                                PageSize::Size4K,
                                self.permission.clone(),
                                Some(batch),
                            )?;

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
        })
    }

    /// 拆分区域为两部分：[start, split_vpn) 和 [split_vpn, end)
    ///
    /// # 参数
    /// - `page_table`: 页表的可变引用
    /// - `split_vpn`: 拆分点（必须在区域范围内，且不等于边界）
    ///
    /// # 返回值
    /// - `Ok((left, right))`: 成功，返回拆分后的两个区域
    /// - `Err(PagingError)`: 拆分失败
    ///
    /// # 注意
    /// - 原区域会被消耗（moved）
    /// - 调用者负责将拆分后的区域插入到 areas 列表中
    /// - 只支持 Framed 映射类型
    pub fn split_at(
        mut self,
        page_table: &mut ActivePageTableInner,
        split_vpn: Vpn,
    ) -> Result<(Self, Self), page_table::PagingError> {
        // 验证拆分点
        if !self.vpn_range.contains(split_vpn) {
            return Err(page_table::PagingError::InvalidAddress);
        }

        if split_vpn == self.vpn_range.start() || split_vpn == self.vpn_range.end() {
            return Err(page_table::PagingError::InvalidAddress);
        }

        // 只支持 Framed 映射
        if self.map_type != MapType::Framed {
            return Err(page_table::PagingError::UnsupportedMapType);
        }

        // 创建左右两个区域的元数据
        let left_range = VpnRange::new(self.vpn_range.start(), split_vpn);
        let right_range = VpnRange::new(split_vpn, self.vpn_range.end());

        // 计算分割点相对于起始的页数
        let left_pages = split_vpn.as_usize() - self.vpn_range.start().as_usize();

        // 如果有文件映射，需要分割文件映射信息
        let left_file = self.file.as_ref().map(|f| MmapFile {
            file: f.file.clone(),
            offset: f.offset,            // 左半部分保持原有偏移量
            len: left_pages * PAGE_SIZE, // 长度调整为左半部分的大小
            prot: f.prot,
            flags: f.flags,
        });

        let right_file = self.file.as_ref().map(|f| MmapFile {
            file: f.file.clone(),
            offset: f.offset + left_pages * PAGE_SIZE, // 偏移量向后移动
            len: f.len - left_pages * PAGE_SIZE,       // 剩余长度
            prot: f.prot,
            flags: f.flags,
        });

        let mut left_area = MappingArea::new(
            left_range,
            self.area_type,
            self.map_type,
            self.permission.clone(),
            left_file,
        );

        let mut right_area = MappingArea::new(
            right_range,
            self.area_type,
            self.map_type,
            self.permission.clone(),
            right_file,
        );

        // 分配帧：遍历原区域的 frames，根据 VPN 分配到左右区域 - 手动迭代并清空
        let vpns: alloc::vec::Vec<Vpn> = self.frames.keys().copied().collect();
        for vpn in vpns {
            if let Some(tracked_frames) = self.frames.remove(&vpn) {
                if vpn < split_vpn {
                    left_area.frames.insert(vpn, tracked_frames);
                } else {
                    right_area.frames.insert(vpn, tracked_frames);
                }
            }
        }

        // 重新建立页表映射
        // 注意：原区域的页表映射仍然存在，我们不需要重新映射
        // 只需要确保 frames 的所有权转移正确

        Ok((left_area, right_area))
    }

    /// 部分修改权限：修改 [start_vpn, end_vpn) 范围的权限
    ///
    /// # 参数
    /// - `page_table`: 页表的可变引用
    /// - `start_vpn`: 起始 VPN（包含）
    /// - `end_vpn`: 结束 VPN（不包含）
    /// - `new_perm`: 新的权限标志
    ///
    /// # 返回值
    /// - `Ok(alloc::vec::Vec<Self>)`: 成功，返回修改后的区域列表
    ///   - 如果整个区域都在范围内：返回包含1个区域的向量（权限已修改）
    ///   - 如果部分在范围内：返回包含2-3个区域的向量（分割后的区域）
    ///
    /// # 注意
    /// - 原区域会被消耗（moved）
    /// - 调用者负责将返回的区域插入到 areas 列表中
    /// - 支持 Framed / Reserved（用于 PROT_NONE）
    pub fn partial_change_permission(
        mut self,
        page_table: &mut ActivePageTableInner,
        start_vpn: Vpn,
        end_vpn: Vpn,
        new_perm: UniversalPTEFlag,
    ) -> Result<alloc::vec::Vec<Self>, page_table::PagingError> {
        let area_start = self.vpn_range.start();
        let area_end = self.vpn_range.end();

        // 计算需要修改权限的实际范围
        let change_start = core::cmp::max(start_vpn, area_start);
        let change_end = core::cmp::min(end_vpn, area_end);

        if change_start >= change_end {
            // 没有重叠，返回原区域
            return Ok(alloc::vec![self]);
        }

        let wants_mapping = new_perm.intersects(
            UniversalPTEFlag::READABLE | UniversalPTEFlag::WRITEABLE | UniversalPTEFlag::EXECUTABLE,
        );

        // 基于修改范围先构造 left/middle/right（并拆分 file 映射元数据）
        let left_range = VpnRange::new(area_start, change_start);
        let middle_range = VpnRange::new(change_start, change_end);
        let right_range = VpnRange::new(change_end, area_end);

        let left_pages = change_start.as_usize() - area_start.as_usize();
        let middle_pages = change_end.as_usize() - change_start.as_usize();

        let left_file = self.file.as_ref().map(|f| MmapFile {
            file: f.file.clone(),
            offset: f.offset,
            len: left_pages * PAGE_SIZE,
            prot: f.prot,
            flags: f.flags,
        });

        let middle_file = self.file.as_ref().map(|f| MmapFile {
            file: f.file.clone(),
            offset: f.offset + left_pages * PAGE_SIZE,
            len: middle_pages * PAGE_SIZE,
            prot: f.prot,
            flags: f.flags,
        });

        let right_file = self.file.as_ref().map(|f| MmapFile {
            file: f.file.clone(),
            offset: f.offset + (left_pages + middle_pages) * PAGE_SIZE,
            len: f.len - (left_pages + middle_pages) * PAGE_SIZE,
            prot: f.prot,
            flags: f.flags,
        });

        let mut left_area = if area_start < change_start {
            Some(MappingArea::new(
                left_range,
                self.area_type,
                self.map_type,
                self.permission.clone(),
                left_file,
            ))
        } else {
            None
        };

        let mut middle_area = MappingArea::new(
            middle_range,
            self.area_type,
            if wants_mapping { MapType::Framed } else { MapType::Reserved },
            new_perm,
            middle_file,
        );

        let mut right_area = if change_end < area_end {
            Some(MappingArea::new(
                right_range,
                self.area_type,
                self.map_type,
                self.permission.clone(),
                right_file,
            ))
        } else {
            None
        };

        match self.map_type {
            MapType::Direct => return Err(page_table::PagingError::UnsupportedMapType),
            MapType::Framed => {
                if wants_mapping {
                    // 仅更新 middle_range 的权限（要求为叶子 PTE：必须有 R/W/X）
                    TlbBatchContext::execute(|batch| {
                        for vpn in VpnRange::new(change_start, change_end) {
                            page_table.update_flags_with_batch(vpn, new_perm, Some(batch))?;
                        }
                        Ok::<(), page_table::PagingError>(())
                    })?;

                    // 把 middle 的 frames 从 self.frames 移交给 middle_area
                    for vpn in VpnRange::new(change_start, change_end) {
                        if let Some(tracked) = self.frames.remove(&vpn) {
                            middle_area.frames.insert(vpn, tracked);
                        }
                    }
                } else {
                    // PROT_NONE：解除 middle_range 的映射并释放 frames
                    TlbBatchContext::execute(|batch| {
                        for vpn in VpnRange::new(change_start, change_end) {
                            self.unmap_one_with_batch(page_table, vpn, Some(batch))?;
                        }
                        Ok::<(), page_table::PagingError>(())
                    })?;
                }

                // 分发剩余 frames 到 left/right
                let vpns: alloc::vec::Vec<Vpn> = self.frames.keys().copied().collect();
                for vpn in vpns {
                    if let Some(tracked) = self.frames.remove(&vpn) {
                        if vpn < change_start {
                            if let Some(ref mut l) = left_area {
                                l.frames.insert(vpn, tracked);
                            }
                        } else if vpn >= change_end {
                            if let Some(ref mut r) = right_area {
                                r.frames.insert(vpn, tracked);
                            }
                        } else {
                            // 中间部分已在上面处理（wants_mapping 时已移交；否则已 unmap）
                        }
                    }
                }
            }
            MapType::Reserved => {
                if wants_mapping {
                    // Reserved -> Framed：为 middle_range 建立页表映射并分配 frames
                    TlbBatchContext::execute(|batch| {
                        for vpn in VpnRange::new(change_start, change_end) {
                            let frame = alloc_frame().ok_or(page_table::PagingError::FrameAllocFailed)?;
                            let ppn = frame.ppn();
                            middle_area.frames.insert(vpn, TrackedFrames::Single(frame));
                            page_table.map_with_batch(
                                vpn,
                                ppn,
                                PageSize::Size4K,
                                middle_area.permission.clone(),
                                Some(batch),
                            )?;
                        }
                        Ok::<(), page_table::PagingError>(())
                    })?;
                } else {
                    // Reserved + PROT_NONE：无需页表操作
                }
            }
        }

        let mut out = alloc::vec::Vec::new();
        if let Some(l) = left_area {
            out.push(l);
        }
        out.push(middle_area);
        if let Some(r) = right_area {
            out.push(r);
        }
        Ok(out)
    }

    /// 部分解除映射：解除 [start_vpn, end_vpn) 范围的映射
    ///
    /// # 参数
    /// - `page_table`: 页表的可变引用
    /// - `start_vpn`: 起始 VPN（包含）
    /// - `end_vpn`: 结束 VPN（不包含）
    ///
    /// # 返回值
    /// - `Ok(Option<(Self, Option<Self>)>)`: 成功
    ///   - `None`: 整个区域被解除映射
    ///   - `Some((left, None))`: 只剩左侧部分
    ///   - `Some((left, Some(right)))`: 中间被解除映射，剩下左右两部分
    ///
    /// # 注意
    /// - 原区域会被消耗（moved）
    pub fn partial_unmap(
        mut self,
        page_table: &mut ActivePageTableInner,
        start_vpn: Vpn,
        end_vpn: Vpn,
    ) -> Result<Option<(Self, Option<Self>)>, page_table::PagingError> {
        let area_start = self.vpn_range.start();
        let area_end = self.vpn_range.end();

        // 计算需要解除映射的实际范围
        let unmap_start = core::cmp::max(start_vpn, area_start);
        let unmap_end = core::cmp::min(end_vpn, area_end);

        if unmap_start >= unmap_end {
            // 没有重叠，返回原区域
            return Ok(Some((self, None)));
        }

        // 解除映射指定范围内的页（Reserved 不需要操作页表）
        if self.map_type != MapType::Reserved {
            TlbBatchContext::execute(|batch| {
                for vpn in VpnRange::new(unmap_start, unmap_end) {
                    self.unmap_one_with_batch(page_table, vpn, Some(batch))?;
                }
                Ok::<(), page_table::PagingError>(())
            })?;
        }

        // 根据解除映射的位置，决定返回什么
        if unmap_start == area_start && unmap_end == area_end {
            // 情况 1: 整个区域被解除映射
            return Ok(None);
        } else if unmap_start == area_start {
            // 情况 2: 解除映射了前半部分，保留 [unmap_end, area_end)
            self.vpn_range = VpnRange::new(unmap_end, area_end);
            return Ok(Some((self, None)));
        } else if unmap_end == area_end {
            // 情况 3: 解除映射了后半部分，保留 [area_start, unmap_start)
            self.vpn_range = VpnRange::new(area_start, unmap_start);
            return Ok(Some((self, None)));
        } else {
            // 情况 4: 解除映射了中间部分，需要拆分为两个区域
            // 保留 [area_start, unmap_start) 和 [unmap_end, area_end)

            let left_range = VpnRange::new(area_start, unmap_start);
            let right_range = VpnRange::new(unmap_end, area_end);

            // 计算左右部分的文件映射信息
            let left_pages = unmap_start.as_usize() - area_start.as_usize();
            let middle_pages = unmap_end.as_usize() - unmap_start.as_usize();

            let left_file = self.file.as_ref().map(|f| MmapFile {
                file: f.file.clone(),
                offset: f.offset, // 左半部分保持原有偏移量
                len: left_pages * PAGE_SIZE,
                prot: f.prot,
                flags: f.flags,
            });

            let right_file = self.file.as_ref().map(|f| MmapFile {
                file: f.file.clone(),
                offset: f.offset + (left_pages + middle_pages) * PAGE_SIZE, // 跳过左半部分和中间被 unmap 的部分
                len: f.len - (left_pages + middle_pages) * PAGE_SIZE,
                prot: f.prot,
                flags: f.flags,
            });

            let mut left_area = MappingArea::new(
                left_range,
                self.area_type,
                self.map_type,
                self.permission.clone(),
                left_file,
            );

            let mut right_area = MappingArea::new(
                right_range,
                self.area_type,
                self.map_type,
                self.permission.clone(),
                right_file,
            );

            // 分配 frames - 手动迭代并清空
            let vpns: alloc::vec::Vec<Vpn> = self.frames.keys().copied().collect();
            for vpn in vpns {
                if let Some(tracked_frames) = self.frames.remove(&vpn) {
                    if vpn < unmap_start {
                        left_area.frames.insert(vpn, tracked_frames);
                    } else if vpn >= unmap_end {
                        right_area.frames.insert(vpn, tracked_frames);
                    }
                    // unmap_start <= vpn < unmap_end 的 frames 已经在 unmap_one 中释放
                }
            }

            return Ok(Some((left_area, Some(right_area))));
        }
    }

    /// 从文件加载数据到已分配的物理页中
    ///
    /// # 错误
    /// - 文件读取失败
    /// - 页面未分配
    pub fn load_from_file(&mut self) -> Result<(), page_table::PagingError> {
        if let Some(ref mmap_file) = self.file {
            let inode = mmap_file
                .file
                .inode()
                .map_err(|_| page_table::PagingError::InvalidAddress)?;
            let start_vpn = self.vpn_range.start();

            for (vpn, tracked_frame) in &self.frames {
                // 计算文件偏移量
                let page_offset = vpn.as_usize() - start_vpn.as_usize();
                let file_offset = mmap_file.offset + page_offset * PAGE_SIZE;

                // 获取物理页并通过内核直接映射访问
                let ppn = match tracked_frame {
                    TrackedFrames::Single(frame) => frame.ppn(),
                    TrackedFrames::Multiple(frames) => frames.first().map(|f| f.ppn()).unwrap(),
                    TrackedFrames::Contiguous(_) => {
                        panic!("当前实现不支持连续帧");
                    }
                };

                let paddr = ppn.start_addr();
                let kernel_vaddr = paddr_to_vaddr(paddr.as_usize());
                let buffer =
                    unsafe { core::slice::from_raw_parts_mut(kernel_vaddr as *mut u8, PAGE_SIZE) };

                // 计算实际读取长度（处理文件末尾）
                let read_len = min(
                    PAGE_SIZE,
                    mmap_file.len.saturating_sub(page_offset * PAGE_SIZE),
                );

                if read_len == 0 {
                    continue; // 超出文件末尾，页面保持清零状态
                }

                // 从文件读取数据
                let actual_read = inode
                    .read_at(file_offset, &mut buffer[..read_len])
                    .map_err(|_| page_table::PagingError::InvalidAddress)?;

                // 部分读取时记录警告（剩余部分保持为零）
                if actual_read < read_len {
                    pr_warn!(
                        "Partial read at offset {}: expected {}, got {}",
                        file_offset,
                        read_len,
                        actual_read
                    );
                }

                // buffer[actual_read..] 保持为零（新分配的物理帧默认清零）
            }
        }
        Ok(())
    }

    /// 将脏页写回文件
    ///
    /// # 参数
    /// - `page_table`: 页表引用，用于检查和清除 Dirty 位
    ///
    /// # 错误
    /// - 文件写入失败
    /// - 部分写入
    pub fn sync_file(
        &self,
        page_table: &mut ActivePageTableInner,
    ) -> Result<(), page_table::PagingError> {
        use crate::arch::mm::TlbBatchContext;

        if let Some(ref mmap_file) = self.file {
            // 只有 MAP_SHARED 映射才需要写回
            if !mmap_file.flags.contains(MapFlags::SHARED) {
                return Ok(());
            }

            let inode = mmap_file
                .file
                .inode()
                .map_err(|_| page_table::PagingError::InvalidAddress)?;
            let start_vpn = self.vpn_range.start();

            TlbBatchContext::execute(|batch| {
                for (vpn, tracked_frame) in &self.frames {
                    // 获取页表项的标志位，检查 Dirty 位
                    let (_, _, flags) = match page_table.walk(*vpn) {
                        Ok(result) => result,
                        Err(_) => continue, // 页面未映射，跳过
                    };

                    if !flags.contains(UniversalPTEFlag::DIRTY) {
                        continue; // 未被修改，跳过
                    }

                    // 计算文件偏移量
                    let page_offset = vpn.as_usize() - start_vpn.as_usize();
                    let file_offset = mmap_file.offset + page_offset * PAGE_SIZE;

                    // 获取物理页内容
                    let ppn = match tracked_frame {
                        TrackedFrames::Single(frame) => frame.ppn(),
                        TrackedFrames::Multiple(frames) => frames.first().map(|f| f.ppn()).unwrap(),
                        TrackedFrames::Contiguous(_) => {
                            panic!("当前实现不支持连续帧");
                        }
                    };

                    let paddr = ppn.start_addr();
                    let kernel_vaddr = paddr_to_vaddr(paddr.as_usize());
                    let buffer = unsafe {
                        core::slice::from_raw_parts(kernel_vaddr as *const u8, PAGE_SIZE)
                    };

                    // 计算实际写入长度（处理文件末尾）
                    let write_len = min(
                        PAGE_SIZE,
                        mmap_file.len.saturating_sub(page_offset * PAGE_SIZE),
                    );

                    if write_len == 0 {
                        continue; // 超出文件范围
                    }

                    // 写回文件
                    let actual_written = inode
                        .write_at(file_offset, &buffer[..write_len])
                        .map_err(|_| page_table::PagingError::InvalidAddress)?;

                    // 检查是否完全写入
                    if actual_written != write_len {
                        pr_err!(
                            "Partial write at offset {}: expected {}, got {}",
                            file_offset,
                            write_len,
                            actual_written
                        );
                        return Err(page_table::PagingError::InvalidAddress);
                    }

                    // 清除 Dirty 位
                    page_table.update_flags_with_batch(
                        *vpn,
                        flags & !UniversalPTEFlag::DIRTY,
                        Some(batch),
                    )?;
                }
                Ok(())
            })
        } else {
            Ok(())
        }
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

        // 解除映射 [new_end, old_end) 范围内的页
        // 对于 4K 页，解除映射顺序不影响正确性
        for i in 0..count {
            let vpn = Vpn::from_usize(new_end.as_usize() + i);
            self.unmap_one(page_table, vpn)?;
        }

        // 更新范围
        self.vpn_range = VpnRange::new(self.vpn_range.start(), new_end);

        Ok(new_end)
    }
}
