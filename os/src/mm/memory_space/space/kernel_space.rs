use super::*;

impl MemorySpace {
    /// 映射内核空间（所有地址空间共享）
    ///
    /// 此方法实现了方案 2（共享页表）的核心逻辑：
    /// 每个用户进程的页表都包含用户空间映射（私有）和内核空间映射（共享）。
    /// 这种设计可以在不切换 `satp` 寄存器的情况下，实现零开销的用户/内核模式切换。
    ///
    /// # 参数
    /// - `include_trampoline`: 是否包含带有内核权限 (U=0) 的跳板页映射
    ///
    /// # 映射内容
    /// 所有映射都使用 **直接映射** (VA = PA + VADDR_START) 且设置 **U=0** 标志:
    /// - 跳板页（可选）：R+X，直接映射
    /// - 内核 .text 段：R+X，直接映射
    /// - 内核 .rodata 段：R，直接映射
    /// - 内核 .data 段：R+W，直接映射
    /// - 内核 .bss.stack 段：R+W，直接映射
    /// - 内核 .bss 段：R+W，直接映射
    /// - 内核堆：R+W，直接映射
    /// - 物理内存：R+W，直接映射
    ///
    /// # 安全性
    /// 所有内核映射都将 U（用户可访问）标志设置为 0，确保即使页表中存在映射，
    /// 用户模式也无法访问内核内存。这是由硬件强制执行的。
    ///
    /// # 架构
    /// 当前实现目标是 RISC-V SV39。其他架构需要相应调整地址范围。
    fn map_kernel_space(&mut self) -> Result<(), PagingError> {
        unsafe extern "C" {
            fn stext();
            fn etext();
            fn srodata();
            fn erodata();
            fn sdata();
            fn edata();
            fn sbss();
            fn ebss();
            fn ekernel();
            fn strampoline();
        }

        // 1. 映射内核 .text 段 (读 + 执行)
        Self::map_kernel_section(
            self,
            stext as usize,
            etext as usize,
            AreaType::KernelText,
            UniversalPTEFlag::kernel_rx(),
        )?;

        // 2. 映射内核 .rodata 段 (只读)
        Self::map_kernel_section(
            self,
            srodata as usize,
            erodata as usize,
            AreaType::KernelRodata,
            UniversalPTEFlag::kernel_r(),
        )?;

        // 3. 映射内核 .data 段 (读-写)
        Self::map_kernel_section(
            self,
            sdata as usize,
            edata as usize,
            AreaType::KernelData,
            UniversalPTEFlag::kernel_rw(),
        )?;

        // 4a. 映射内核启动栈 (.bss.stack section)
        Self::map_kernel_section(
            self,
            edata as usize, // .bss.stack 从 edata 开始
            sbss as usize,  // .bss.stack 在 sbss 结束
            AreaType::KernelStack,
            UniversalPTEFlag::kernel_rw(),
        )?;

        // 4b. 映射内核 .bss 段
        Self::map_kernel_section(
            self,
            sbss as usize,
            ebss as usize,
            AreaType::KernelBss,
            UniversalPTEFlag::kernel_rw(),
        )?;

        // 4c. 映射内核堆
        Self::map_kernel_section(
            self,
            ebss as usize,    // sheap
            ekernel as usize, // eheap
            AreaType::KernelHeap,
            UniversalPTEFlag::kernel_rw(),
        )?;

        // 5. 映射物理内存（从 ekernel 到物理内存末端的直接映射）
        //
        // 评测机可能使用较大的 -m（如 1G/4G），QEMU 把 DTB 放在 RAM 顶端（-m 4G 时约
        // 0xbfe0_0000）。若只映射到编译期 MEMORY_END（128MB），切换页表后解引用 DTB 会缺页，
        // 且帧分配器也无法管理评测所需的大内存。改用设备树报告的真实 DRAM 末端，并与帧分配器
        // 范围保持一致（见 mm::init）。
        let ekernel_paddr = unsafe { crate::arch::va_to_pa(VA::from_usize(ekernel as usize)) };

        let phys_mem_end_paddr = match crate::device::device_tree::dram_info() {
            Some((dram_start, dram_size)) => dram_start.saturating_add(dram_size),
            None => MEMORY_END,
        };

        // LoongArch virt 报告超大 RAM 窗口，用 4K 页映射多 GB 极慢；且 LoongArch 经 DMW 硬件
        // 直映射访问物理内存（不经页表）。把直映射窗口 cap 在 1GiB，与帧分配器一致。RISC-V 不
        // 设上限，覆盖全部 DRAM（确保 DTB 也在范围内）。
        #[cfg(target_arch = "loongarch64")]
        let phys_mem_end_paddr = {
            const MAX_KERNEL_DIRECT_MAP_BYTES: usize = 1024 * 1024 * 1024; // 1GiB
            let cap_end = ekernel_paddr
                .as_usize()
                .saturating_add(MAX_KERNEL_DIRECT_MAP_BYTES);
            phys_mem_end_paddr.min(cap_end)
        };

        let phys_mem_start_vaddr = crate::arch::pa_to_va(ekernel_paddr);
        let phys_mem_end_vaddr = crate::arch::pa_to_va(PA::from_usize(phys_mem_end_paddr));

        let phys_mem_start = Vpn::from_addr_ceil(phys_mem_start_vaddr);
        let phys_mem_end = Vpn::from_addr_floor(phys_mem_end_vaddr);
        let mut phys_mem_area = MappingArea::new(
            VpnRange::new(phys_mem_start, phys_mem_end),
            AreaType::KernelHeap,
            MapType::Direct,
            UniversalPTEFlag::kernel_rw(),
            None, // 内核直接映射，无文件
        );

        phys_mem_area.map(&mut self.page_table)?;
        self.areas.push(phys_mem_area);

        // 确保 DTB 被映射（即使在上面 cap 了物理内存直映射窗口之后）。
        //
        // RISC-V 依赖页表直映射访问 DTB；LoongArch 用 DMW 硬件窗口，无需在页表里额外映射。
        // 正常情况下 DTB 落在上面的物理内存直映射范围内，此处仅作兜底。
        #[cfg(not(target_arch = "loongarch64"))]
        {
            let dtb_paddr =
                crate::device::device_tree::DTP.load(core::sync::atomic::Ordering::Acquire);
            if dtb_paddr != 0 {
                let dtb_start = dtb_paddr & !(PAGE_SIZE - 1);
                // 仅当 DTB 不在（可能被 cap 的）物理内存直映射窗口内时才单独映射。
                if dtb_start < ekernel_paddr.as_usize() || dtb_start >= phys_mem_end_paddr {
                    let dtb_vaddr = crate::arch::pa_to_va(PA::from_usize(dtb_start));
                    let vpn = Vpn::from_addr_floor(dtb_vaddr);
                    if self.find_area(vpn).is_none() {
                        // 2MiB 足够覆盖 DTB（通常 < 1MiB），且对齐简单。
                        self.map_mmio_region(dtb_vaddr, 2 * 1024 * 1024)?;
                    }
                }
            }
        }

        // 暂时移除自动 MMIO 映射
        // // 6. 映射 MMIO 区域
        // for &(_device, mmio_base, mmio_size) in crate::config::MMIO {
        //     let mmio_vaddr = crate::arch::pa_to_va(mmio_base);
        //     self.map_mmio_region(mmio_vaddr, mmio_size)?;
        // }

        Ok(())
    }

    /// 创建内核内存空间
    ///
    /// 这将创建一个完整的内核地址空间，包括跳板页、内核段（.text、.rodata、.data、.bss、堆）以及直接映射的
    /// 物理内存。供内核线程和系统初始化时使用。
    pub fn new_kernel() -> Result<Self, PagingError> {
        let mut space = MemorySpace::new()?;

        // 映射所有内核空间（包括带内核权限的跳板页）
        space.map_kernel_space()?;

        Ok(space)
    }

    /// 辅助函数：映射一个内核段
    fn map_kernel_section(
        space: &mut MemorySpace,
        start: usize,
        end: usize,
        area_type: AreaType,
        flags: UniversalPTEFlag,
    ) -> Result<(), PagingError> {
        let vpn_start = Vpn::from_addr_floor(VA::from_usize(start));
        let vpn_end = Vpn::from_addr_ceil(VA::from_usize(end));

        let mut area = MappingArea::new(
            VpnRange::new(vpn_start, vpn_end),
            area_type,
            MapType::Direct,
            flags,
            None, // Direct 映射无文件
        );

        area.map(&mut space.page_table)?;
        space.areas.push(area);
        Ok(())
    }

    /// 映射一个 MMIO 区域
    ///
    /// # 参数
    /// - `addr`: MMIO 设备的虚拟地址（已通过 pa_to_va 转换）
    /// - `size`: MMIO 区域的大小（字节）
    ///
    /// # 返回
    /// - `Ok(())`: 映射成功
    /// - `Err(PagingError)`: 映射失败
    pub fn map_mmio_region(&mut self, addr: VA, size: usize) -> Result<(), PagingError> {
        let vpn_start = Vpn::from_addr_floor(addr);
        let vpn_end = Vpn::from_addr_ceil(VA::from_usize(addr.as_usize() + size));

        let mut area = MappingArea::new(
            VpnRange::new(vpn_start, vpn_end),
            AreaType::KernelMmio,
            MapType::Direct,
            UniversalPTEFlag::kernel_rw(),
            None, // MMIO 映射无文件
        );

        area.map(&mut self.page_table)?;
        self.areas.push(area);
        Ok(())
    }

    /// 进程手动映射MMIO区域
    pub fn map_mmio(&mut self, paddr: PA, size: usize) -> Result<VA, PagingError> {
        // 将物理地址转换为虚拟地址
        let vaddr = crate::arch::pa_to_va(paddr);
        let vaddr_usize = vaddr.as_usize();

        // 计算VPN范围
        let vpn_start = Vpn::from_addr_floor(vaddr);
        let vpn_end = Vpn::from_addr_ceil(VA::from_usize(vaddr_usize + size));

        // 检查是否已经映射
        let mut some_unmapped = false;
        let mut some_mapped = false;
        for vpn in VpnRange::new(vpn_start, vpn_end) {
            if let Some(area) = self.find_area(vpn) {
                if area.area_type() != AreaType::KernelMmio {
                    // 已经被映射为其他类型,这是一个错误
                    return Err(PagingError::AlreadyMapped);
                }
                some_mapped = true;
            } else {
                some_unmapped = true;
            }
        }

        // 如果已经完全映射,直接返回虚拟地址 (幂等)
        if some_mapped && !some_unmapped {
            return Ok(vaddr);
        }

        // 如果部分映射，当前实现会创建重叠区域，这是一个bug。
        // 暂时禁止部分重叠映射，要求用户精确映射。
        if some_mapped && some_unmapped {
            // TODO: 实现对部分映射区域的扩展或合并
            return Err(PagingError::AlreadyMapped);
        }

        // 如果没有映射,调用map_mmio_region进行映射
        self.map_mmio_region(VA::from_usize(vaddr_usize), size)?;
        Ok(vaddr)
    }

    /// 进程手动取消映射MMIO区域
    pub fn unmap_mmio(&mut self, vaddr: VA, size: usize) -> Result<(), PagingError> {
        // 计算VPN范围
        let vpn_start = Vpn::from_addr_floor(vaddr);
        let vpn_end = Vpn::from_addr_ceil(VA::from_usize(vaddr.as_usize() + size));

        // 使用 BTreeSet 以提高去重效率
        let mut areas_to_remove = alloc::collections::BTreeSet::new();
        let unmap_vpn_range = VpnRange::new(vpn_start, vpn_end);

        let mut current_vpn = vpn_start;
        while current_vpn < vpn_end {
            if let Some(area) = self.find_area(current_vpn) {
                // 验证这是一个MMIO区域
                if area.area_type() != AreaType::KernelMmio {
                    return Err(PagingError::InvalidAddress);
                }

                // 安全性检查：确保要取消映射的区域完全覆盖了此area，防止意外删除
                let area_range = area.vpn_range();
                if !unmap_vpn_range.contains_range(&area_range) {
                    // 错误：尝试部分取消映射，当前不支持
                    // TODO: 如果需要，可以实现区域分割逻辑
                    return Err(PagingError::InvalidAddress);
                }

                // 记录需要移除的区域起始VPN
                areas_to_remove.insert(area.vpn_range().start());
                // 优化：跳到此区域之后继续查找
                current_vpn = area.vpn_range().end();
            } else {
                // 优化：跳到下一页
                current_vpn = Vpn::from_usize(current_vpn.as_usize() + 1);
            }
        }

        // 移除所有相关的MMIO区域
        for vpn in areas_to_remove {
            self.remove_area(vpn)?;
        }

        Ok(())
    }
}
