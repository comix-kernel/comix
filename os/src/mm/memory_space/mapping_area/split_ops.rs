use super::*;

impl MappingArea {
    /// 克隆元数据，但不克隆帧
    /// 用于写时复制（COW）的 fork
    pub fn clone_metadata(&self) -> Self {
        MappingArea {
            vpn_range: self.vpn_range,
            area_type: self.area_type,
            map_type: self.map_type,
            permission: self.permission,
            frames: BTreeMap::new(), // 不克隆帧
            // fork 时复制文件映射信息（MAP_SHARED 和 MAP_PRIVATE 都需要）
            file: self.file.as_ref().map(|f| MmapFile {
                file: f.file.clone(),
                offset: f.offset,
                len: f.len,
                prot: f.prot,
                flags: f.flags,
            }),
            shared: self.shared.clone(),
            shared_page_offset: self.shared_page_offset,
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
                            let src_va = crate::arch::pa_to_va(src_ppn.start_addr());
                            let dst_va = crate::arch::pa_to_va(new_ppn.start_addr());

                            core::ptr::copy_nonoverlapping(
                                src_va.as_usize() as *const u8,
                                dst_va.as_usize() as *mut u8,
                                crate::config::PAGE_SIZE,
                            );
                        }

                        // 建立页表映射
                        page_table.map_with_batch(
                            *vpn,
                            new_ppn,
                            PageSize::Size4K,
                            self.permission,
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
                                let src_va = crate::arch::pa_to_va(src_ppn.start_addr());
                                let dst_va = crate::arch::pa_to_va(new_ppn.start_addr());

                                core::ptr::copy_nonoverlapping(
                                    src_va.as_usize() as *const u8,
                                    dst_va.as_usize() as *mut u8,
                                    crate::config::PAGE_SIZE,
                                );
                            }

                            // 建立页表映射
                            page_table.map_with_batch(
                                *vpn,
                                new_ppn,
                                PageSize::Size4K,
                                self.permission,
                                Some(batch),
                            )?;

                            new_frames.push(new_frame);
                        }

                        new_area
                            .frames
                            .insert(*vpn, TrackedFrames::Multiple(new_frames));
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
        _page_table: &mut ActivePageTableInner,
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
            self.permission,
            left_file,
        );
        left_area.shared = self.shared.clone();
        left_area.shared_page_offset = self.shared_page_offset;

        let mut right_area = MappingArea::new(
            right_range,
            self.area_type,
            self.map_type,
            self.permission,
            right_file,
        );
        right_area.shared = self.shared.clone();
        right_area.shared_page_offset = self.shared_page_offset + left_pages;

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
                self.permission,
                left_file,
            ))
        } else {
            None
        };
        if let Some(ref mut l) = left_area {
            l.shared = self.shared.clone();
            l.shared_page_offset = self.shared_page_offset;
        }

        let mut middle_area = MappingArea::new(
            middle_range,
            self.area_type,
            if wants_mapping {
                MapType::Framed
            } else {
                MapType::Reserved
            },
            new_perm,
            middle_file,
        );
        middle_area.shared = self.shared.clone();
        middle_area.shared_page_offset = self.shared_page_offset + left_pages;

        let mut right_area = if change_end < area_end {
            Some(MappingArea::new(
                right_range,
                self.area_type,
                self.map_type,
                self.permission,
                right_file,
            ))
        } else {
            None
        };
        if let Some(ref mut r) = right_area {
            r.shared = self.shared.clone();
            r.shared_page_offset = self.shared_page_offset + left_pages + middle_pages;
        }

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
                            let frame =
                                alloc_frame().ok_or(page_table::PagingError::FrameAllocFailed)?;
                            let ppn = frame.ppn();
                            middle_area.frames.insert(vpn, TrackedFrames::Single(frame));
                            page_table.map_with_batch(
                                vpn,
                                ppn,
                                PageSize::Size4K,
                                middle_area.permission,
                                Some(batch),
                            )?;
                        }
                        Ok::<(), page_table::PagingError>(())
                    })?;
                } else {
                    // Reserved + PROT_NONE：无需页表操作
                }
            }
            MapType::Shared => {
                if !wants_mapping {
                    return Err(page_table::PagingError::UnsupportedMapType);
                }
                TlbBatchContext::execute(|batch| {
                    for vpn in VpnRange::new(change_start, change_end) {
                        page_table.update_flags_with_batch(vpn, new_perm, Some(batch))?;
                    }
                    Ok::<(), page_table::PagingError>(())
                })?;
                middle_area.map_type = MapType::Shared;
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
            Ok(None)
        } else if unmap_start == area_start {
            // 情况 2: 解除映射了前半部分，保留 [unmap_end, area_end)
            self.vpn_range = VpnRange::new(unmap_end, area_end);
            self.shared_page_offset += unmap_end.as_usize() - area_start.as_usize();
            Ok(Some((self, None)))
        } else if unmap_end == area_end {
            // 情况 3: 解除映射了后半部分，保留 [area_start, unmap_start)
            self.vpn_range = VpnRange::new(area_start, unmap_start);
            Ok(Some((self, None)))
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
                self.permission,
                left_file,
            );

            let mut right_area = MappingArea::new(
                right_range,
                self.area_type,
                self.map_type,
                self.permission,
                right_file,
            );
            left_area.shared = self.shared.clone();
            left_area.shared_page_offset = self.shared_page_offset;
            right_area.shared = self.shared.clone();
            right_area.shared_page_offset = self.shared_page_offset + left_pages + middle_pages;

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

            Ok(Some((left_area, Some(right_area))))
        }
    }
}
