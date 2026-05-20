use super::*;

impl MappingArea {
    pub fn vpn_range(&self) -> VpnRange {
        self.vpn_range
    }

    pub fn permission(&self) -> UniversalPTEFlag {
        self.permission
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
    pub(super) fn map_one_with_batch(
        &mut self,
        page_table: &mut ActivePageTableInner,
        vpn: Vpn,
        batch: Option<&mut TlbBatchContext>,
    ) -> Result<(), page_table::PagingError> {
        let ppn = match self.map_type {
            MapType::Direct => {
                // 对于直接映射，VPN 等于 PPN + 偏移量
                let vaddr = vpn.start_addr();
                let paddr = unsafe { crate::arch::va_to_pa(vaddr) };
                Ppn::from_addr_floor(paddr)
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

        page_table.map_with_batch(vpn, ppn, PageSize::Size4K, self.permission, batch)?;
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
    pub(super) fn unmap_one_with_batch(
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
                let dst_va = crate::arch::pa_to_va(PA::from_usize(paddr));
                let dst = dst_va.as_usize() as *mut u8;
                let src = data.as_ptr().add(copied);
                core::ptr::copy_nonoverlapping(src, dst, to_copy);
            }

            copied += to_copy;
        }
    }
}
