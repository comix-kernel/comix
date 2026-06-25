use super::*;

impl MemorySpace {
    /// 创建一个新的空内存空间
    pub fn new() -> Result<Self, PagingError> {
        Ok(MemorySpace {
            page_table: ActivePageTableInner::new()?,
            areas: Vec::new(),
            heap_start: None,
        })
    }

    /// 返回页表的引用
    pub fn page_table(&self) -> &ActivePageTableInner {
        &self.page_table
    }

    /// 返回页表的可变引用
    pub fn page_table_mut(&mut self) -> &mut ActivePageTableInner {
        &mut self.page_table
    }

    /// 返回根页表的物理页号 (PPN)
    pub fn root_ppn(&self) -> Ppn {
        self.page_table.root_ppn()
    }

    /// 返回所有映射区域的引用
    pub fn areas(&self) -> &Vec<MappingArea> {
        &self.areas
    }

    /// 返回所有映射区域的可变引用
    pub fn areas_mut(&mut self) -> &mut Vec<MappingArea> {
        &mut self.areas
    }

    /// 获取当前的 brk 值（堆的当前结束地址）
    ///
    /// # 返回值
    /// - 如果堆区域存在，返回堆的结束地址（current brk）
    /// - 如果堆区域不存在，返回堆的起始地址
    /// - 如果堆未初始化，返回 None
    pub fn current_brk(&self) -> Option<VA> {
        self.areas
            .iter()
            .find(|a| a.area_type() == AreaType::UserHeap)
            .map(|a| a.vpn_range().end().start_addr())
            .or_else(|| self.heap_start.map(|vpn| vpn.start_addr()))
    }

    /// 创建一个新的用户地址空间，并克隆当前地址空间的内核映射。
    ///
    /// 语义与 `from_elf()` 内部“只复制内核区域”的逻辑一致，便于在 execve 等路径中复用。
    pub fn new_user_with_kernel_mappings() -> Result<Self, PagingError> {
        let current_space = crate::kernel::current_memory_space();
        let current_locked = current_space.lock();

        let mut space = MemorySpace::new()?;
        for area in current_locked.areas.iter() {
            let is_kernel = matches!(
                area.area_type(),
                AreaType::KernelText
                    | AreaType::KernelRodata
                    | AreaType::KernelData
                    | AreaType::KernelBss
                    | AreaType::KernelStack
                    | AreaType::KernelHeap
                    | AreaType::KernelMmio
            );
            if !is_kernel {
                continue;
            }
            space.clone_direct_area(area)?;
        }

        // Userspace rt_sigreturn trampoline (Linux ABI).
        space.map_user_sigreturn_trampoline()?;

        Ok(space)
    }

    /// 设置用户堆的起始地址（brk 的下界）。
    ///
    /// 注意：这里只设置固定的 heap_start，不会创建/扩展 UserHeap 映射区域。
    pub fn set_heap_start(&mut self, heap_start: Vpn) {
        self.heap_start = Some(heap_start);
    }

    pub(super) fn clone_direct_area(&mut self, area: &MappingArea) -> Result<(), PagingError> {
        let mut new_area = area.clone_metadata();

        #[cfg(target_arch = "loongarch64")]
        if new_area.area_type() == AreaType::KernelMmio {
            self.areas.push(new_area);
            return Ok(());
        }

        new_area.map(&mut self.page_table)?;
        self.areas.push(new_area);
        Ok(())
    }

    pub(super) fn map_user_sigreturn_trampoline(&mut self) -> Result<(), PagingError> {
        let start = USER_SIGRETURN_TRAMPOLINE;
        let end = start
            .checked_add(PAGE_SIZE)
            .ok_or(PagingError::InvalidAddress)?;
        let vpn_range = VpnRange::new(
            Vpn::from_addr_floor(VA::from_usize(start)),
            Vpn::from_addr_ceil(VA::from_usize(end)),
        );

        // If already mapped (layout differences), don't fail hard.
        if self
            .areas
            .iter()
            .any(|a| a.vpn_range().overlaps(&vpn_range))
        {
            return Ok(());
        }

        let code = crate::arch::kernel_sigreturn_trampoline_bytes();
        self.insert_framed_area(
            vpn_range,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rx(),
            Some(code),
            None,
        )?;

        Ok(())
    }

    /// 从当前地址空间中向指定虚拟地址写入字节序列（跨页安全）。
    pub fn write_bytes_at(&mut self, va: usize, bytes: &[u8]) -> Result<(), PagingError> {
        if bytes.is_empty() {
            return Ok(());
        }

        let mut written = 0usize;
        while written < bytes.len() {
            let cur_va = va.checked_add(written).ok_or(PagingError::InvalidAddress)?;
            let paddr = self
                .page_table
                .translate(VA::from_usize(cur_va))
                .ok_or(PagingError::InvalidAddress)?;
            let paddr_usize = paddr.as_usize();
            let page_base = paddr_usize & !(PAGE_SIZE - 1);
            let page_off = paddr_usize & (PAGE_SIZE - 1);

            let take = core::cmp::min(bytes.len() - written, PAGE_SIZE - page_off);
            let dst =
                (crate::arch::pa_to_va(PA::from_usize(page_base)).as_usize() + page_off) as *mut u8;
            unsafe {
                core::ptr::copy_nonoverlapping(bytes[written..].as_ptr(), dst, take);
            }
            written += take;
        }

        Ok(())
    }

    /// 从指定虚拟地址读取字节序列（跨页安全）。
    pub fn read_bytes_at(&self, va: usize, out: &mut [u8]) -> Result<(), PagingError> {
        if out.is_empty() {
            return Ok(());
        }

        let mut read = 0usize;
        while read < out.len() {
            let cur_va = va.checked_add(read).ok_or(PagingError::InvalidAddress)?;
            let paddr = self
                .page_table
                .translate(VA::from_usize(cur_va))
                .ok_or(PagingError::InvalidAddress)?;
            let paddr_usize = paddr.as_usize();
            let page_base = paddr_usize & !(PAGE_SIZE - 1);
            let page_off = paddr_usize & (PAGE_SIZE - 1);

            let take = core::cmp::min(out.len() - read, PAGE_SIZE - page_off);
            let src = (crate::arch::pa_to_va(PA::from_usize(page_base)).as_usize() + page_off)
                as *const u8;
            unsafe {
                core::ptr::copy_nonoverlapping(src, out[read..].as_mut_ptr(), take);
            }
            read += take;
        }
        Ok(())
    }

    pub fn read_u64_at(&self, va: usize) -> Result<u64, PagingError> {
        let mut buf = [0u8; 8];
        self.read_bytes_at(va, &mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }

    pub fn read_i64_at(&self, va: usize) -> Result<i64, PagingError> {
        let mut buf = [0u8; 8];
        self.read_bytes_at(va, &mut buf)?;
        Ok(i64::from_le_bytes(buf))
    }

    pub fn write_usize_at(&mut self, va: usize, value: usize) -> Result<(), PagingError> {
        self.write_bytes_at(va, &value.to_ne_bytes())
    }

    /// 插入一个新的映射区域并检测重叠
    ///
    /// # 错误
    /// 如果该区域与现有区域重叠，则返回错误
    pub fn insert_area(&mut self, mut area: MappingArea) -> Result<(), PagingError> {
        // 1. 检查重叠
        for existing in &self.areas {
            if existing.vpn_range().overlaps(&area.vpn_range()) {
                return Err(PagingError::AlreadyMapped);
            }
        }

        // 2. 映射到页表（如果失败，area 会自动被丢弃）
        area.map(&mut self.page_table)?;

        // 3. 添加到区域列表
        self.areas.push(area);

        Ok(())
    }

    /// 插入一个帧映射区域，并可选择复制数据
    pub fn insert_framed_area(
        &mut self,
        vpn_range: VpnRange,
        area_type: AreaType,
        flags: UniversalPTEFlag,
        data: Option<&[u8]>,
        file: Option<MmapFile>,
    ) -> Result<(), PagingError> {
        let area = MappingArea::new(vpn_range, area_type, MapType::Framed, flags, file);

        // 检查重叠并插入 (insert_area 会在内部进行页面映射)
        self.insert_area(area)?;

        // 如果提供了数据，则复制数据（访问 self.areas 中最新添加的区域）
        if let Some(data) = data {
            let area = self.areas.last_mut().unwrap();
            area.copy_data(&mut self.page_table, data, 0);
        }

        Ok(())
    }

    /// 插入一个“保留”区域（不建立页表映射）
    ///
    /// 用于 mmap(PROT_NONE) / guard page 场景：需要占位并参与重叠检查，
    /// 但不能创建 RISC-V 不合法的“无 R/W/X 叶子页表项”。
    pub fn insert_reserved_area(
        &mut self,
        vpn_range: VpnRange,
        area_type: AreaType,
        flags: UniversalPTEFlag,
        file: Option<MmapFile>,
    ) -> Result<(), PagingError> {
        let area = MappingArea::new(vpn_range, area_type, MapType::Reserved, flags, file);
        self.insert_area(area)?;
        Ok(())
    }

    /// 插入 SysV shared memory 映射区域。
    pub fn insert_shared_area(
        &mut self,
        vpn_range: VpnRange,
        flags: UniversalPTEFlag,
        segment: alloc::sync::Arc<crate::ipc::ShmSegment>,
    ) -> Result<(), PagingError> {
        let area = MappingArea::new_shared(vpn_range, flags, segment);
        self.insert_area(area)?;
        Ok(())
    }

    /// 插入一个帧映射区域，并可选择复制数据（带偏移量）
    pub fn insert_framed_area_with_offset(
        &mut self,
        vpn_range: VpnRange,
        area_type: AreaType,
        flags: UniversalPTEFlag,
        data: Option<&[u8]>,
        offset: usize,
        file: Option<MmapFile>,
    ) -> Result<(), PagingError> {
        let area = MappingArea::new(vpn_range, area_type, MapType::Framed, flags, file);

        // 检查重叠并插入 (insert_area 会在内部进行页面映射)
        self.insert_area(area)?;

        // 如果提供了数据，则复制数据（访问 self.areas 中最新添加的区域）
        if let Some(data) = data {
            let area = self.areas.last_mut().unwrap();
            area.copy_data(&mut self.page_table, data, offset);
        }

        Ok(())
    }

    /// 查找包含给定 VPN 的区域
    pub fn find_area(&self, vpn: Vpn) -> Option<&MappingArea> {
        self.areas
            .iter()
            .find(|area| area.vpn_range().contains(vpn))
    }

    /// 查找包含给定 VPN 的区域（可变）
    pub fn find_area_mut(&mut self, vpn: Vpn) -> Option<&mut MappingArea> {
        self.areas
            .iter_mut()
            .find(|area| area.vpn_range().contains(vpn))
    }

    /// 获取所有 MMIO 映射区域（用于测试）
    #[cfg(test)]
    pub fn get_mmio_areas(&self) -> Vec<&MappingArea> {
        self.areas
            .iter()
            .filter(|area| area.area_type() == AreaType::KernelMmio)
            .collect()
    }

    /// 通过 VPN 移除并取消映射一个区域
    pub fn remove_area(&mut self, vpn: Vpn) -> Result<(), PagingError> {
        if let Some(pos) = self.areas.iter().position(|a| a.vpn_range().contains(vpn)) {
            let mut area = self.areas.remove(pos);
            area.unmap(&mut self.page_table)?;
            Ok(())
        } else {
            Err(PagingError::NotMapped)
        }
    }

    /// 克隆内存空间（用于 fork 系统调用）
    ///
    /// # 注意
    /// - 直接映射是共享的（不复制）
    /// - 帧映射是深层复制的
    pub fn clone_for_fork(&self) -> Result<Self, PagingError> {
        let mut new_space = MemorySpace::new()?;
        new_space.heap_start = self.heap_start;

        for area in self.areas.iter() {
            match area.map_type() {
                MapType::Direct => {
                    // 直接映射：克隆元数据并重新映射到新的页表
                    new_space.clone_direct_area(area)?;
                }
                MapType::Framed => {
                    // 帧映射：深层复制数据
                    let new_area = area.clone_with_data(&mut new_space.page_table)?;
                    new_space.areas.push(new_area);
                }
                MapType::Reserved => {
                    // 保留区：仅克隆元数据（无页表映射、无帧）
                    let new_area = area.clone_metadata();
                    new_space.areas.push(new_area);
                }
                MapType::Shared => {
                    let mut new_area = area.clone_metadata();
                    new_area.map(&mut new_space.page_table)?;
                    new_space.areas.push(new_area);
                }
            }
        }

        Ok(new_space)
    }

    pub fn translate(&self, vaddr: VA) -> Option<PA> {
        self.page_table.translate(vaddr)
    }
}

/// 为 MemorySpace 实现 Drop trait
///
/// 在进程退出时，自动写回所有文件映射的脏页
impl Drop for MemorySpace {
    fn drop(&mut self) {
        // 遍历所有区域，尽力写回文件映射
        for area in &self.areas {
            if let Err(e) = area.sync_file(&mut self.page_table) {
                pr_warn!(
                    "Failed to sync file mapping during MemorySpace drop: {:?}",
                    e
                );
                // 继续处理其他区域，不能 panic
            }
        }
        // 其余清理工作由各字段的 Drop 自动完成
    }
}
