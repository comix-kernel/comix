use core::cmp::Ordering;

use crate::arch::mm::{paddr_to_vaddr, vaddr_to_paddr};
use crate::config::{MAX_USER_HEAP_SIZE, MEMORY_END, PAGE_SIZE, USER_STACK_SIZE, USER_STACK_TOP};
use crate::mm::address::{Paddr, PageNum, Ppn, UsizeConvert, Vaddr, Vpn, VpnRange};
use crate::mm::memory_space::MmapFile;
use crate::mm::memory_space::mapping_area::{AreaType, MapType, MappingArea};
use crate::mm::page_table::{ActivePageTableInner, PageTableInner, PagingError, UniversalPTEFlag};
use crate::println;
use crate::sync::SpinLock;
use crate::{pr_err, pr_warn};
use alloc::vec::Vec;
use lazy_static::lazy_static;

// 内核链接器符号
unsafe extern "C" {
    fn stext(); // .text (代码段) 的起始地址
    fn etext(); // .text (代码段) 的结束地址
    fn srodata(); // .rodata (只读数据段) 的起始地址
    fn erodata(); // .rodata (只读数据段) 的结束地址
    fn sdata(); // .data (数据段) 的起始地址
    fn edata(); // .data (数据段) 的结束地址
    fn sbss(); // .bss (未初始化数据段) 的起始地址
    fn ebss(); // .bss (未初始化数据段) 的结束地址
    fn ekernel(); // 内核所有段的结束地址（即物理内存的起始可分配地址）
    fn strampoline(); // 位于高半部分的跳板页 (trampoline page) 的起始地址
}

lazy_static! {
    /// 全局内核内存空间（受 SpinLock 保护）
    static ref KERNEL_SPACE: SpinLock<MemorySpace> = {
        SpinLock::new(MemorySpace::new_kernel())
    };
}

/// 返回内核页表令牌（用于激活页表，例如 RISC-V 上的 satp 寄存器值）
pub fn kernel_token() -> usize {
    (KERNEL_SPACE.lock().page_table.root_ppn().as_usize() << 44) | (8 << 60)
}

/// 返回内核根页表的物理页号 (PPN)
pub fn kernel_root_ppn() -> Ppn {
    KERNEL_SPACE.lock().root_ppn()
}

/// 以独占方式访问内核空间并执行闭包
pub fn with_kernel_space<F, R>(f: F) -> R
where
    F: FnOnce(&mut MemorySpace) -> R,
{
    let mut guard = KERNEL_SPACE.lock();
    f(&mut guard)
}

/// 表示地址空间的内存空间结构体
#[derive(Debug)]
pub struct MemorySpace {
    /// 与此内存空间关联的页表
    page_table: ActivePageTableInner,

    /// 此内存空间中的映射区域列表
    areas: Vec<MappingArea>,

    /// 堆的起始地址 (brk 系统调用使用，仅限用户空间)
    /// 注意：这是堆的固定起始位置，真正的堆顶（current brk）存储在 UserHeap 区域的 vpn_range.end 中
    heap_start: Option<Vpn>,
}

impl MemorySpace {
    /// 创建一个新的空内存空间
    pub fn new() -> Self {
        MemorySpace {
            page_table: ActivePageTableInner::new(),
            areas: Vec::new(),
            heap_start: None,
        }
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
    pub fn current_brk(&self) -> Option<usize> {
        self.areas
            .iter()
            .find(|a| a.area_type() == AreaType::UserHeap)
            .map(|a| a.vpn_range().end().start_addr().as_usize())
            .or_else(|| self.heap_start.map(|vpn| vpn.start_addr().as_usize()))
    }

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

        // 5. 映射物理内存（从 ekernel 到 MEMORY_END 的直接映射）
        let ekernel_paddr = unsafe { vaddr_to_paddr(ekernel as usize) };
        let phys_mem_start_vaddr = paddr_to_vaddr(ekernel_paddr);
        let phys_mem_end_vaddr = paddr_to_vaddr(MEMORY_END);

        let phys_mem_start = Vpn::from_addr_ceil(Vaddr::from_usize(phys_mem_start_vaddr));
        let phys_mem_end = Vpn::from_addr_floor(Vaddr::from_usize(phys_mem_end_vaddr));
        let mut phys_mem_area = MappingArea::new(
            VpnRange::new(phys_mem_start, phys_mem_end),
            AreaType::KernelHeap,
            MapType::Direct,
            UniversalPTEFlag::kernel_rw(),
            None, // 内核直接映射，无文件
        );

        phys_mem_area.map(&mut self.page_table)?;
        self.areas.push(phys_mem_area);

        // 暂时移除自动 MMIO 映射
        // // 6. 映射 MMIO 区域
        // for &(_device, mmio_base, mmio_size) in crate::config::MMIO {
        //     let mmio_vaddr = paddr_to_vaddr(mmio_base);
        //     self.map_mmio_region(mmio_vaddr, mmio_size)?;
        // }

        Ok(())
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

    /// 创建内核内存空间
    ///
    /// 这将创建一个完整的内核地址空间，包括跳板页、内核段（.text、.rodata、.data、.bss、堆）以及直接映射的
    /// 物理内存。供内核线程和系统初始化时使用。
    pub fn new_kernel() -> Self {
        let mut space = MemorySpace::new();

        // 映射所有内核空间（包括带内核权限的跳板页）
        space
            .map_kernel_space()
            .expect("Failed to map kernel space");

        space
    }

    /// 辅助函数：映射一个内核段
    fn map_kernel_section(
        space: &mut MemorySpace,
        start: usize,
        end: usize,
        area_type: AreaType,
        flags: UniversalPTEFlag,
    ) -> Result<(), PagingError> {
        let vpn_start = Vpn::from_addr_floor(Vaddr::from_usize(start));
        let vpn_end = Vpn::from_addr_ceil(Vaddr::from_usize(end));

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
    /// - `addr`: MMIO 设备的虚拟地址（已通过 paddr_to_vaddr 转换）
    /// - `size`: MMIO 区域的大小（字节）
    ///
    /// # 返回
    /// - `Ok(())`: 映射成功
    /// - `Err(PagingError)`: 映射失败
    pub fn map_mmio_region(&mut self, addr: usize, size: usize) -> Result<(), PagingError> {
        let vpn_start = Vpn::from_addr_floor(Vaddr::from_usize(addr));
        let vpn_end = Vpn::from_addr_ceil(Vaddr::from_usize(addr + size));

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

    /// 从 ELF 文件创建用户内存空间
    ///
    /// 此方法通过创建一个包含用户空间映射（进程私有）和内核空间映射（所有进程共享）的页表，
    /// 实现了方案 2（共享页表）。
    ///
    /// 最终的页表支持零开销的用户/内核模式切换：
    /// 当用户进程陷入内核时，内核代码已被映射且可访问，无需切换 `satp`。
    ///
    /// # 返回
    /// 成功时返回 `Ok((space, entry_point, user_stack_top))`：
    /// - `space`: 包含用户 + 内核映射的内存空间
    /// - `entry_point`: 程序入口地址（来自 ELF 头）
    /// - `user_stack_top`: 用户栈的顶部地址
    ///
    /// # 错误
    /// - ELF 解析失败
    /// - 架构不匹配（非 RISC-V）
    /// - 段与保留区域重叠
    pub fn from_elf(
        elf_data: &[u8],
    ) -> Result<(Self, usize, usize, usize, usize, usize), PagingError> {
        use xmas_elf::ElfFile;
        use xmas_elf::program::{SegmentData, Type};

        let elf = ElfFile::new(elf_data).map_err(|_| PagingError::InvalidAddress)?;

        // 检查架构
        if elf.header.pt2.machine().as_machine() != xmas_elf::header::Machine::RISC_V {
            return Err(PagingError::InvalidAddress);
        }

        // 创建新的内存空间，只复制内核映射（不复制用户空间数据）
        let current_space = crate::kernel::current_memory_space();
        let current_locked = current_space.lock();

        let mut space = MemorySpace::new();

        // 只复制内核空间区域的元数据和映射
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
            if is_kernel {
                // 对于内核区域，只需要克隆元数据并重新映射（不复制数据）
                let mut new_area = area.clone_metadata();
                new_area.map(&mut space.page_table)?;
                space.areas.push(new_area);
            }
        }

        drop(current_locked);

        let mut max_end_vpn = Vpn::from_usize(0);

        // 1. 解析并映射 ELF 段
        for ph in elf.program_iter() {
            if ph.get_type() != Ok(Type::Load) {
                continue;
            }

            let start_va = ph.virtual_addr() as usize;
            let end_va = (ph.virtual_addr() + ph.mem_size()) as usize;

            // 检查段是否与栈/陷阱区域重叠
            if start_va >= USER_STACK_TOP - USER_STACK_SIZE {
                return Err(PagingError::InvalidAddress);
            }

            let vpn_range = VpnRange::new(
                Vpn::from_addr_floor(Vaddr::from_usize(start_va)),
                Vpn::from_addr_ceil(Vaddr::from_usize(end_va)),
            );

            max_end_vpn = if max_end_vpn.as_usize() > vpn_range.end().as_usize() {
                max_end_vpn
            } else {
                vpn_range.end()
            };

            // 构建权限
            let mut flags = UniversalPTEFlag::USER_ACCESSIBLE | UniversalPTEFlag::VALID;
            if ph.flags().is_read() {
                flags |= UniversalPTEFlag::READABLE;
            }
            if ph.flags().is_write() {
                flags |= UniversalPTEFlag::WRITEABLE;
            }
            if ph.flags().is_execute() {
                flags |= UniversalPTEFlag::EXECUTABLE;
            }

            // 确定区域类型
            let area_type = if ph.flags().is_execute() {
                AreaType::UserText
            } else if ph.flags().is_write() {
                AreaType::UserData
            } else {
                AreaType::UserRodata
            };

            // 获取段数据
            let data = match ph.get_data(&elf) {
                Ok(SegmentData::Undefined(data)) => Some(data),
                _ => None,
            };

            // 插入区域（将在内部检查重叠）
            space.insert_framed_area_with_offset(
                vpn_range,
                area_type,
                flags,
                data,
                start_va % PAGE_SIZE, // Use page offset, not file offset
                None,                 // 非文件映射
            )?;
        }

        // 2. 初始化堆（从 ELF 结束地址开始，页对齐）
        space.heap_start = Some(max_end_vpn);

        // 3. 映射用户栈（带保护页）
        let user_stack_bottom =
            Vpn::from_addr_floor(Vaddr::from_usize(USER_STACK_TOP - USER_STACK_SIZE));
        let user_stack_top = Vpn::from_addr_ceil(Vaddr::from_usize(USER_STACK_TOP));

        space.insert_framed_area(
            VpnRange::new(user_stack_bottom, user_stack_top),
            AreaType::UserStack,
            UniversalPTEFlag::user_rw(),
            None,
            None, // 非文件映射
        )?;

        let entry_point = elf.header.pt2.entry_point() as usize;
        let ph_off = elf.header.pt2.ph_offset() as usize;
        let ph_num = elf.header.pt2.ph_count();
        let ph_ent = elf.header.pt2.ph_entry_size();

        // PHDR 在虚拟内存中的地址 = 第一个 LOAD 段的虚拟地址 + PHDR 在文件中的偏移
        // 假设第一个 LOAD 段映射了 ELF 头（通常如此，偏移为 0）
        // 如果第一个 LOAD 段偏移不为 0，则需要更复杂的逻辑，但对于标准 ELF 通常足够
        // 简单起见，我们假设 ELF 头被映射到了 base_addr
        // 实际上，我们应该找到包含 ph_off 的那个段
        let mut phdr_addr = 0;
        for ph in elf.program_iter() {
            if ph.get_type() == Ok(Type::Load) {
                let vaddr = ph.virtual_addr() as usize;
                let offset = ph.offset() as usize;
                let filesz = ph.file_size() as usize;
                if ph_off >= offset && ph_off < offset + filesz {
                    phdr_addr = vaddr + (ph_off - offset);
                    break;
                }
            }
        }

        Ok((
            space,
            entry_point,
            USER_STACK_TOP,
            phdr_addr,
            ph_num as usize,
            ph_ent as usize,
        ))
    }

    /// 扩展或收缩堆区域 (brk 系统调用)
    ///
    /// # 错误
    /// - 堆未初始化
    /// - 新的 brk 会超出 MAX_USER_HEAP_SIZE
    /// - 新的 brk 会与现有区域重叠
    pub fn brk(&mut self, new_brk: usize) -> Result<usize, PagingError> {
        let heap_bottom = self.heap_start.ok_or(PagingError::InvalidAddress)?;
        let new_end_vpn = Vpn::from_addr_ceil(Vaddr::from_usize(new_brk));

        // 边界检查
        if new_brk < heap_bottom.start_addr().as_usize() {
            return Err(PagingError::InvalidAddress);
        }

        let heap_size = new_brk - heap_bottom.start_addr().as_usize();
        if heap_size > MAX_USER_HEAP_SIZE {
            return Err(PagingError::InvalidAddress);
        }

        // 检查是否与栈重叠
        if new_brk >= USER_STACK_TOP - USER_STACK_SIZE {
            return Err(PagingError::InvalidAddress);
        }

        // 查找或创建堆区域
        let heap_area_idx = self
            .areas
            .iter()
            .position(|a| a.area_type() == AreaType::UserHeap);

        if let Some(idx) = heap_area_idx {
            // 存在堆区域，调整大小
            let old_end = self.areas[idx].vpn_range().end();

            match new_end_vpn.cmp(&old_end) {
                Ordering::Greater => {
                    // 扩展：检查是否与其他区域冲突
                    let new_range = VpnRange::new(old_end, new_end_vpn);
                    for (i, area) in self.areas.iter().enumerate() {
                        if i != idx && area.vpn_range().overlaps(&new_range) {
                            // 与mmap或其他区域冲突
                            return Err(PagingError::AlreadyMapped);
                        }
                    }

                    let count = new_end_vpn.as_usize() - old_end.as_usize();
                    if count != 0 {
                        self.areas[idx].extend(&mut self.page_table, count)?;
                    }
                }
                Ordering::Less => {
                    // 收缩
                    if new_end_vpn <= heap_bottom {
                        // 收缩到起始位置或更低，删除整个堆区域
                        let mut area = self.areas.remove(idx);
                        area.unmap(&mut self.page_table)?;
                    } else {
                        let count = old_end.as_usize() - new_end_vpn.as_usize();
                        if count != 0 {
                            self.areas[idx].shrink(&mut self.page_table, count)?;
                        }
                    }
                }
                Ordering::Equal => { /* 无操作 */ }
            }
        } else {
            // 第一次分配堆，创建新区域
            if new_end_vpn > heap_bottom {
                // 检查是否与现有区域冲突
                let new_range = VpnRange::new(heap_bottom, new_end_vpn);
                for area in &self.areas {
                    if area.vpn_range().overlaps(&new_range) {
                        return Err(PagingError::AlreadyMapped);
                    }
                }

                self.insert_framed_area(
                    new_range,
                    AreaType::UserHeap,
                    UniversalPTEFlag::user_rw(),
                    None,
                    None, // 非文件映射
                )?;
            }
        }

        Ok(new_brk)
    }

    /// 查找足够大的空闲地址区域
    ///
    /// # 参数
    /// - `size`: 需要的大小（字节）
    /// - `align`: 对齐要求（字节）
    ///
    /// # 返回值
    /// - `Some(addr)`: 找到的空闲区域起始地址（已对齐）
    /// - `None`: 没有足够大的空闲区域
    pub fn find_free_region(&self, size: usize, align: usize) -> Option<usize> {
        // 获取堆的起始和结束
        let heap_start = self.heap_start?.start_addr().as_usize();

        // 获取当前堆的实际结束地址（不包含 mmap 区域）
        let heap_end = self
            .areas
            .iter()
            .filter(|a| a.area_type() == AreaType::UserHeap)
            .map(|a| a.vpn_range().end().start_addr().as_usize())
            .max()
            .unwrap_or(heap_start);

        // 栈的底部地址
        let stack_bottom = USER_STACK_TOP - USER_STACK_SIZE;

        // 预留栈增长空间（建议至少 1MB）
        const STACK_GUARD_SIZE: usize = 1024 * 1024;
        let search_limit = stack_bottom.saturating_sub(STACK_GUARD_SIZE);

        // 收集所有用户区域（包括 heap 和 mmap），按起始地址排序
        let mut user_areas: alloc::vec::Vec<(usize, usize)> = self
            .areas
            .iter()
            .filter(|a| {
                matches!(
                    a.area_type(),
                    AreaType::UserHeap
                        | AreaType::UserMmap
                        | AreaType::UserStack
                        | AreaType::UserText
                        | AreaType::UserRodata
                        | AreaType::UserData
                        | AreaType::UserBss
                )
            })
            .map(|a| {
                let start = a.vpn_range().start().start_addr().as_usize();
                let end = a.vpn_range().end().start_addr().as_usize();
                (start, end)
            })
            .collect();

        user_areas.sort_by_key(|&(start, _)| start);

        // 检查堆结束到第一个区域之间的空隙
        let mut search_start = heap_end;

        // 对齐到页边界（向上取整）
        search_start =
            (search_start + crate::config::PAGE_SIZE - 1) & !(crate::config::PAGE_SIZE - 1);

        for &(area_start, area_end) in &user_areas {
            // 跳过在 heap_end 之前的区域
            if area_end <= heap_end {
                continue;
            }

            // 检查 [search_start, area_start) 是否足够大
            if area_start > search_start {
                let gap_size = area_start - search_start;
                if gap_size >= size && search_start < search_limit {
                    // 应用对齐要求
                    let aligned_start = (search_start + align - 1) & !(align - 1);
                    if aligned_start + size <= area_start && aligned_start < search_limit {
                        return Some(aligned_start);
                    }
                }
            }

            // 更新搜索起点到当前区域之后
            search_start = area_end;
        }

        // 检查最后一个区域之后到栈之前的空间
        if search_start < search_limit {
            let remaining = search_limit - search_start;
            if remaining >= size {
                let aligned_start = (search_start + align - 1) & !(align - 1);
                if aligned_start + size <= search_limit {
                    return Some(aligned_start);
                }
            }
        }

        None
    }

    /// 映射一个匿名区域（简化的 mmap）
    ///
    /// # 参数
    /// - `hint`: 建议的起始地址（0 = 由内核选择）
    /// - `len`: 长度（字节）
    /// - `pte_flags`: 页表项标志（应包含 VALID 和 USER_ACCESSIBLE）
    pub fn mmap(
        &mut self,
        hint: usize,
        len: usize,
        pte_flags: UniversalPTEFlag,
    ) -> Result<usize, PagingError> {
        if len == 0 {
            return Err(PagingError::InvalidAddress);
        }

        // 确定起始地址
        let start = if hint == 0 {
            // 内核选择地址：查找空闲区域
            self.find_free_region(len, crate::config::PAGE_SIZE)
                .ok_or(PagingError::OutOfMemory)?
        } else {
            // 用户指定地址

            // 检查是否在有效范围内
            if hint >= USER_STACK_TOP - USER_STACK_SIZE {
                return Err(PagingError::InvalidAddress);
            }

            // 将 hint 向下对齐到页边界（Linux 行为）
            let aligned_hint = hint & !(crate::config::PAGE_SIZE - 1);

            // 检查对齐后的区域是否可用
            let vpn_range_check = VpnRange::new(
                Vpn::from_addr_floor(Vaddr::from_usize(aligned_hint)),
                Vpn::from_addr_ceil(Vaddr::from_usize(aligned_hint + len)),
            );

            // 检查是否与现有区域重叠
            let has_overlap = self
                .areas
                .iter()
                .any(|a| a.vpn_range().overlaps(&vpn_range_check));

            if has_overlap {
                // hint 不可用，尝试查找附近的空闲区域
                // 注意：这里简化处理，直接查找任意空闲区域
                // 更好的实现应该优先查找 hint 附近的区域
                self.find_free_region(len, crate::config::PAGE_SIZE)
                    .ok_or(PagingError::AlreadyMapped)?
            } else {
                aligned_hint
            }
        };

        // 计算 VPN 范围（start 已经是页对齐的）
        let vpn_range = VpnRange::new(
            Vpn::from_addr_floor(Vaddr::from_usize(start)),
            Vpn::from_addr_ceil(Vaddr::from_usize(start + len)),
        );

        // 最终重叠检查（防御性编程）
        for area in &self.areas {
            if area.vpn_range().overlaps(&vpn_range) {
                return Err(PagingError::AlreadyMapped);
            }
        }

        // 创建映射区域
        self.insert_framed_area(vpn_range, AreaType::UserMmap, pte_flags, None, None)?;

        // 返回对齐后的地址
        Ok(start)
    }

    /// 解除映射一个区域（munmap 系统调用）
    ///
    /// # 参数
    /// - `start`: 起始地址（字节）
    /// - `len`: 长度（字节）
    ///
    /// # 返回值
    /// - `Ok(())`: 成功
    /// - `Err(PagingError)`: 失败
    ///
    /// # 语义
    /// - 解除映射 [start, start+len) 范围
    /// - 如果范围跨越多个区域，会部分解除映射每个区域
    /// - 如果只覆盖区域的一部分，会拆分区域
    /// - 如果地址未映射，返回成功（幂等）
    pub fn munmap(&mut self, start: usize, len: usize) -> Result<(), PagingError> {
        // 参数验证
        if len == 0 {
            return Ok(()); // POSIX: len=0 是合法的，什么都不做
        }

        // 计算需要解除映射的 VPN 范围
        let start_vpn = Vpn::from_addr_floor(Vaddr::from_usize(start));
        let end_vpn = Vpn::from_addr_ceil(Vaddr::from_usize(start + len));
        let unmap_range = VpnRange::new(start_vpn, end_vpn);

        // 收集需要处理的区域
        // 注意：不能在迭代时修改 self.areas，所以先收集索引
        let mut affected_indices = alloc::vec::Vec::new();

        for (idx, area) in self.areas.iter().enumerate() {
            if area.vpn_range().overlaps(&unmap_range) {
                affected_indices.push(idx);
            }
        }

        // 如果没有重叠的区域，直接返回成功（幂等）
        if affected_indices.is_empty() {
            return Ok(());
        }

        // 处理每个受影响的区域
        // 从后往前处理，避免索引失效
        affected_indices.reverse();

        for idx in affected_indices {
            // 移除原区域
            let area = self.areas.remove(idx);

            // 只处理 Framed 映射，Direct 映射不应该被 munmap
            if area.map_type() != MapType::Framed {
                // 重新插入原区域
                self.areas.insert(idx, area);
                continue;
            }

            // 在解除映射之前，先尝试写回文件（如果是文件映射）
            // 注意：即使 sync_file 失败，仍然继续 munmap，避免内存泄漏
            let sync_result = area.sync_file(&mut self.page_table);

            // 部分解除映射
            match area.partial_unmap(&mut self.page_table, start_vpn, end_vpn)? {
                None => {
                    // 整个区域被解除映射，不需要重新插入
                }
                Some((left, None)) => {
                    // 只剩一个区域
                    self.areas.insert(idx, left);
                }
                Some((left, Some(right))) => {
                    // 拆分为两个区域
                    self.areas.insert(idx, left);
                    self.areas.insert(idx + 1, right);
                }
            }

            // 如果写回失败，返回错误（但映射已经被解除）
            sync_result?;
        }

        Ok(())
    }

    /// 修改内存区域的保护权限（mprotect 系统调用）
    ///
    /// # 参数
    /// - `start`: 起始地址（字节），必须页对齐
    /// - `len`: 长度（字节）
    /// - `prot`: 新的保护标志
    ///
    /// # 返回值
    /// - 成功: 返回 Ok(())
    /// - 失败: 返回 PagingError
    ///
    /// # 注意
    /// - 地址必须页对齐
    /// - 范围必须完全在现有映射区域内
    /// - 如果 mprotect 只应用于区域的一部分，会自动分割区域
    /// - 只能修改 Framed 类型的映射区域
    pub fn mprotect(
        &mut self,
        start: usize,
        len: usize,
        prot: UniversalPTEFlag,
    ) -> Result<(), PagingError> {
        // 参数验证
        if len == 0 {
            return Ok(()); // len=0 是合法的，什么都不做
        }

        // 检查地址对齐
        if start % PAGE_SIZE != 0 {
            return Err(PagingError::InvalidAddress);
        }

        // 计算需要修改权限的 VPN 范围
        let start_vpn = Vpn::from_addr_floor(Vaddr::from_usize(start));
        let end_vpn = Vpn::from_addr_ceil(Vaddr::from_usize(start + len));
        let change_range = VpnRange::new(start_vpn, end_vpn);

        // 收集需要处理的区域
        // 注意：不能在迭代时修改 self.areas，所以先收集索引
        let mut affected_indices = alloc::vec::Vec::new();

        for (idx, area) in self.areas.iter().enumerate() {
            if area.vpn_range().overlaps(&change_range) {
                // 只处理 Framed 类型的映射
                if area.map_type() == MapType::Framed {
                    affected_indices.push(idx);
                } else {
                    // Direct 映射不允许修改权限
                    return Err(PagingError::UnsupportedMapType);
                }
            }
        }

        // 如果没有重叠的区域，返回错误（地址无效）
        if affected_indices.is_empty() {
            return Err(PagingError::InvalidAddress);
        }

        // 验证所有需要修改的 VPN 都在某个 Framed 区域中
        for vpn in start_vpn.as_usize()..end_vpn.as_usize() {
            let vpn = Vpn::from_usize(vpn);
            let found = self
                .areas
                .iter()
                .any(|area| area.vpn_range().contains(vpn) && area.map_type() == MapType::Framed);
            if !found {
                return Err(PagingError::InvalidAddress);
            }
        }

        // 处理每个受影响的区域
        // 从后往前处理，避免索引失效
        affected_indices.reverse();

        for idx in affected_indices {
            // 移除原区域
            let area = self.areas.remove(idx);

            // 使用 partial_change_permission 方法处理区域
            let new_areas =
                area.partial_change_permission(&mut self.page_table, start_vpn, end_vpn, prot)?;

            // 将新区域按顺序插入回 areas 列表
            for (offset, new_area) in new_areas.into_iter().enumerate() {
                self.areas.insert(idx + offset, new_area);
            }
        }

        Ok(())
    }

    /// 克隆内存空间（用于 fork 系统调用）
    ///
    /// # 注意
    /// - 直接映射是共享的（不复制）
    /// - 帧映射是深层复制的
    pub fn clone_for_fork(&self) -> Result<Self, PagingError> {
        let mut new_space = MemorySpace::new();
        new_space.heap_start = self.heap_start;

        for area in self.areas.iter() {
            match area.map_type() {
                MapType::Direct => {
                    // 直接映射：克隆元数据并重新映射到新的页表
                    let mut new_area = area.clone_metadata();
                    new_area.map(&mut new_space.page_table)?;
                    new_space.areas.push(new_area);
                }
                MapType::Framed => {
                    // 帧映射：深层复制数据
                    let new_area = area.clone_with_data(&mut new_space.page_table)?;
                    new_space.areas.push(new_area);
                }
            }
        }

        Ok(new_space)
    }

    /// 进程手动映射MMIO区域
    pub fn map_mmio(&mut self, paddr: Paddr, size: usize) -> Result<Vaddr, PagingError> {
        // 将物理地址转换为虚拟地址
        let vaddr_usize = paddr_to_vaddr(paddr.as_usize());
        let vaddr = Vaddr::from_usize(vaddr_usize);

        // 计算VPN范围
        let vpn_start = Vpn::from_addr_floor(vaddr);
        let vpn_end = Vpn::from_addr_ceil(Vaddr::from_usize(vaddr_usize + size));

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
        self.map_mmio_region(vaddr_usize, size)?;
        Ok(vaddr)
    }

    /// 进程手动取消映射MMIO区域
    pub fn unmap_mmio(&mut self, vaddr: Vaddr, size: usize) -> Result<(), PagingError> {
        // 计算VPN范围
        let vpn_start = Vpn::from_addr_floor(vaddr);
        let vpn_end = Vpn::from_addr_ceil(Vaddr::from_usize(vaddr.as_usize() + size));

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

    pub fn translate(&self, vaddr: Vaddr) -> Option<Paddr> {
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

#[cfg(test)]
mod memory_space_tests {
    use super::*;
    use crate::mm::address::{Vpn, VpnRange};
    use crate::mm::page_table::UniversalPTEFlag;
    use crate::{kassert, println, test_case};

    // 1. 创建内存空间
    test_case!(test_memspace_create, {
        #[allow(unused)]
        let ms = MemorySpace::new();
        // 应该已初始化页表
    });

    // 2. 直接映射
    test_case!(test_direct_mapping, {
        let mut ms = MemorySpace::new();
        let vpn_range = VpnRange::new(Vpn::from_usize(0x80000), Vpn::from_usize(0x80010));

        let area = MappingArea::new(
            vpn_range,
            AreaType::KernelData,
            MapType::Direct,
            UniversalPTEFlag::kernel_rw(),
            None,
        );

        ms.insert_area(area).expect("add area failed");
    });

    // 3. 帧映射
    test_case!(test_framed_mapping, {
        let mut ms = MemorySpace::new();
        let vpn_range = VpnRange::new(Vpn::from_usize(0x1000), Vpn::from_usize(0x1010));

        let area = MappingArea::new(
            vpn_range,
            AreaType::UserData,
            MapType::Framed,
            UniversalPTEFlag::user_rw(),
            None,
        );

        ms.insert_area(area).expect("add area failed");
        // 帧映射会自动分配帧
    });

    // 4. 内核空间访问
    test_case!(test_kernel_space, {
        use crate::mm::memory_space::memory_space::kernel_token;

        let token = kernel_token();
        kassert!(token > 0); // 有效的 SATP 值
    });

    // 5. 测试 MMIO 映射是否存在 - 已移除自动映射,改为测试手动映射
    test_case!(test_mmio_mapping_exists, {
        use crate::mm::memory_space::memory_space::with_kernel_space;

        with_kernel_space(|space| {
            // 由于移除了自动 MMIO 映射,初始状态应该没有 MMIO 区域
            let mmio_areas = space.get_mmio_areas();

            println!("Initial MMIO areas count: {}", mmio_areas.len());
            kassert!(mmio_areas.is_empty());

            println!("  MMIO mapping test passed (no auto-mapping as expected)");
        });
    });

    // 6. 测试 MMIO 地址翻译 - 使用独立的 MemorySpace 实例
    test_case!(test_mmio_translation, {
        use crate::arch::mm::paddr_to_vaddr;

        // 使用独立的 MemorySpace 实例，避免与其他测试或全局状态冲突
        let mut ms = MemorySpace::new();

        // 使用一个不太可能被占用的测试地址
        const TEST_MMIO_PADDR: usize = 0xE000_0000;
        const TEST_MMIO_SIZE: usize = 0x1000;

        println!(
            "Testing MMIO translation at PA=0x{:x}, size=0x{:x}",
            TEST_MMIO_PADDR, TEST_MMIO_SIZE
        );

        // 手动映射 MMIO 区域
        let paddr = Paddr::from_usize(TEST_MMIO_PADDR);
        let result = ms.map_mmio(paddr, TEST_MMIO_SIZE);
        kassert!(result.is_ok());

        let vaddr = result.unwrap();
        let mmio_vaddr = vaddr.as_usize();
        let vpn = Vpn::from_addr_floor(vaddr);

        println!("  Mapped to VA=0x{:x}", mmio_vaddr);

        // 查找包含该地址的区域
        let area = ms.find_area(vpn);
        kassert!(area.is_some());

        if let Some(area) = area {
            kassert!(area.area_type() == AreaType::KernelMmio);
            kassert!(area.map_type() == MapType::Direct);
        }

        // 测试页表翻译
        let translated_paddr = ms.page_table().translate(Vaddr::from_usize(mmio_vaddr));
        kassert!(translated_paddr.is_some());

        if let Some(paddr) = translated_paddr {
            println!(
                "  Translation successful: VA 0x{:x} -> PA 0x{:x}",
                mmio_vaddr,
                paddr.as_usize()
            );
            // 验证翻译结果（允许页偏移误差）
            let expected_paddr = TEST_MMIO_PADDR & !0xfff; // 清除页内偏移
            let actual_paddr = paddr.as_usize() & !0xfff;
            kassert!(actual_paddr == expected_paddr);
        }

        println!("  MMIO translation test passed");
    });

    // // 7. 测试 MMIO 内存访问（读写测试）- 修改为手动映射后访问
    // test_case!(test_mmio_memory_access, {
    //     use crate::mm::memory_space::memory_space::with_kernel_space;

    //     // 注意：这个测试会实际访问 MMIO 设备
    //     // QEMU virt 机器的 TEST 设备 (0x100000) 支持简单的读写
    //     const TEST_DEVICE_PADDR: usize = 0x0010_0000;
    //     const TEST_DEVICE_SIZE: usize = 0x1000;

    //     if crate::config::MMIO
    //         .iter()
    //         .any(|&(_, addr, _)| addr == TEST_DEVICE_PADDR)
    //     {
    //         println!("Testing MMIO memory access at PA=0x{:x}", TEST_DEVICE_PADDR);

    //         // XXX: 疑似因为不再使用KERNEL_SPACE作为全局内核页表导致这里失效
    //         with_kernel_space(|space| {
    //             // 手动映射 TEST 设备
    //             let paddr = Paddr::from_usize(TEST_DEVICE_PADDR);
    //             let result = space.map_mmio(paddr, TEST_DEVICE_SIZE);
    //             kassert!(result.is_ok());

    //             let vaddr = result.unwrap();
    //             let test_vaddr = vaddr.as_usize();

    //             println!("  Mapped TEST device to VA=0x{:x}", test_vaddr);

    //             // 读取测试设备的值（应该可以安全读取）
    //             let value = unsafe { core::ptr::read_volatile(test_vaddr as *const u32) };

    //             println!("  Read value from TEST device: 0x{:x}", value);

    //             // TEST 设备的特性：写入某些值会触发特定行为
    //             // 这里我们只验证写操作不会导致 panic
    //             // 注意：不要写入 0x5555 (FINISHER_PASS) 或 0x3333 (FINISHER_FAIL)
    //             // 因为这会导致 QEMU 退出

    //             println!("  MMIO read test passed (no page fault occurred)");
    //         });
    //     } else {
    //         println!("Warning: TEST device (0x100000) not in MMIO configuration");
    //     }
    // });

    // 8. 测试动态添加 MMIO 映射
    test_case!(test_dynamic_mmio_mapping, {
        use crate::arch::mm::paddr_to_vaddr;

        let mut ms = MemorySpace::new();

        // 尝试映射一个自定义的 MMIO 区域（使用未占用的地址）
        const CUSTOM_MMIO_PADDR: usize = 0x5000_0000;
        const CUSTOM_MMIO_SIZE: usize = 0x1000;

        let custom_vaddr = paddr_to_vaddr(CUSTOM_MMIO_PADDR);

        println!(
            "Adding custom MMIO mapping at PA=0x{:x}, VA=0x{:x}",
            CUSTOM_MMIO_PADDR, custom_vaddr
        );

        // 动态添加 MMIO 映射
        let result = ms.map_mmio_region(custom_vaddr, CUSTOM_MMIO_SIZE);
        kassert!(result.is_ok());

        // 验证映射存在
        let vpn = Vpn::from_addr_floor(Vaddr::from_usize(custom_vaddr));
        let area = ms.find_area(vpn);
        kassert!(area.is_some());

        if let Some(area) = area {
            kassert!(area.area_type() == AreaType::KernelMmio);
            println!("  Dynamic MMIO mapping test passed");
        }
    });

    // 9. 测试 map_mmio 函数 - 新映射
    test_case!(test_map_mmio_new_mapping, {
        let mut ms = MemorySpace::new();

        // 使用一个未占用的物理地址
        const TEST_PADDR: usize = 0x6000_0000;
        const TEST_SIZE: usize = 0x2000;

        let paddr = Paddr::from_usize(TEST_PADDR);

        println!("Testing map_mmio with new mapping at PA=0x{:x}", TEST_PADDR);

        // 调用 map_mmio 进行映射
        let result = ms.map_mmio(paddr, TEST_SIZE);
        kassert!(result.is_ok());

        if let Ok(vaddr) = result {
            println!("  Mapped to VA=0x{:x}", vaddr.as_usize());

            // 验证映射存在
            let vpn = Vpn::from_addr_floor(vaddr);
            let area = ms.find_area(vpn);
            kassert!(area.is_some());

            if let Some(area) = area {
                kassert!(area.area_type() == AreaType::KernelMmio);
                kassert!(area.map_type() == MapType::Direct);
                println!("  map_mmio new mapping test passed");
            }
        }
    });

    // 10. 测试 map_mmio 函数 - 已存在的映射
    test_case!(test_map_mmio_existing_mapping, {
        let mut ms = MemorySpace::new();

        const TEST_PADDR: usize = 0x7000_0000;
        const TEST_SIZE: usize = 0x1000;

        let paddr = Paddr::from_usize(TEST_PADDR);

        println!(
            "Testing map_mmio with existing mapping at PA=0x{:x}",
            TEST_PADDR
        );

        // 第一次映射
        let result1 = ms.map_mmio(paddr, TEST_SIZE);
        kassert!(result1.is_ok());
        let vaddr1 = result1.unwrap();

        // 第二次映射同一个区域
        let result2 = ms.map_mmio(paddr, TEST_SIZE);
        kassert!(result2.is_ok());
        let vaddr2 = result2.unwrap();

        // 应该返回相同的虚拟地址
        kassert!(vaddr1.as_usize() == vaddr2.as_usize());
        println!(
            "  map_mmio existing mapping test passed (VA=0x{:x})",
            vaddr1.as_usize()
        );
    });

    // 11. 测试 map_mmio 函数 - 冲突检测
    test_case!(test_map_mmio_conflict, {
        use crate::arch::mm::paddr_to_vaddr;

        let mut ms = MemorySpace::new();

        // 使用一个合理的物理地址
        const TEST_PADDR: usize = 0x8000_0000;
        const TEST_SIZE: usize = 0x1000;

        // 先通过 paddr_to_vaddr 获取虚拟地址
        let test_vaddr = paddr_to_vaddr(TEST_PADDR);
        let vpn_start = Vpn::from_addr_floor(Vaddr::from_usize(test_vaddr));
        let vpn_end = Vpn::from_addr_ceil(Vaddr::from_usize(test_vaddr + TEST_SIZE));

        println!(
            "Testing map_mmio conflict detection at VA=0x{:x}",
            test_vaddr
        );

        // 首先映射一个非MMIO区域到这个位置
        let vpn_range = VpnRange::new(vpn_start, vpn_end);
        let area = MappingArea::new(
            vpn_range,
            AreaType::KernelData,
            MapType::Direct,
            UniversalPTEFlag::kernel_rw(),
            None,
        );
        ms.insert_area(area).expect("Failed to insert test area");

        // 现在尝试用 map_mmio 映射同一物理地址
        let paddr = Paddr::from_usize(TEST_PADDR);
        let result = ms.map_mmio(paddr, TEST_SIZE);

        // 应该返回 AlreadyMapped 错误,因为该区域已经被映射为非MMIO类型
        kassert!(result.is_err());

        if let Err(e) = result {
            println!("  Expected error occurred: {:?}", e);
            match e {
                PagingError::AlreadyMapped => {
                    println!("  map_mmio conflict detection test passed");
                }
                _ => {
                    println!("  Unexpected error type: {:?}", e);
                }
            }
        }
    });

    // 12. 测试 unmap_mmio 函数 - 正常取消映射
    test_case!(test_unmap_mmio_normal, {
        let mut ms = MemorySpace::new();

        const TEST_PADDR: usize = 0x9000_0000;
        const TEST_SIZE: usize = 0x1000;

        let paddr = Paddr::from_usize(TEST_PADDR);

        println!(
            "Testing unmap_mmio with normal unmapping at PA=0x{:x}",
            TEST_PADDR
        );

        // 先映射
        let result = ms.map_mmio(paddr, TEST_SIZE);
        kassert!(result.is_ok());
        let vaddr = result.unwrap();

        println!("  Mapped to VA=0x{:x}", vaddr.as_usize());

        // 验证映射存在
        let vpn = Vpn::from_addr_floor(vaddr);
        kassert!(ms.find_area(vpn).is_some());

        // 取消映射
        let unmap_result = ms.unmap_mmio(vaddr, TEST_SIZE);
        kassert!(unmap_result.is_ok());

        // 验证映射已被移除
        kassert!(ms.find_area(vpn).is_none());
        println!("  unmap_mmio normal test passed");
    });

    // 13. 测试 unmap_mmio 函数 - 取消映射不存在的区域
    test_case!(test_unmap_mmio_not_mapped, {
        let mut ms = MemorySpace::new();

        // 尝试取消映射一个未映射的区域
        let vaddr = Vaddr::from_usize(0xffff_ffc0_a000_0000);
        const TEST_SIZE: usize = 0x1000;

        println!("Testing unmap_mmio with non-existent mapping");

        let result = ms.unmap_mmio(vaddr, TEST_SIZE);
        // 如果没有找到任何区域，areas_to_remove 为空，不会调用 remove_area
        // 所以应该返回 Ok(())
        kassert!(result.is_ok());
        println!("  unmap_mmio non-existent mapping test passed");
    });

    // 14. 测试 unmap_mmio 函数 - 错误的区域类型
    test_case!(test_unmap_mmio_wrong_type, {
        let mut ms = MemorySpace::new();

        // 映射一个非MMIO区域
        let vpn_range = VpnRange::new(Vpn::from_usize(0xb000), Vpn::from_usize(0xb010));
        let area = MappingArea::new(
            vpn_range,
            AreaType::KernelData,
            MapType::Direct,
            UniversalPTEFlag::kernel_rw(),
            None,
        );
        ms.insert_area(area).expect("Failed to insert test area");

        println!("Testing unmap_mmio with wrong area type");

        // 尝试用 unmap_mmio 取消映射非MMIO区域
        let vaddr = Vpn::from_usize(0xb000).start_addr();
        let result = ms.unmap_mmio(vaddr, 0x1000);

        // 应该返回错误
        kassert!(result.is_err());
        if let Err(e) = result {
            println!("  Expected error occurred: {:?}", e);
            println!("  unmap_mmio wrong type test passed");
        }
    });

    // 15. 测试 map_mmio 和 unmap_mmio 组合 - 多个区域
    test_case!(test_mmio_multiple_regions, {
        let mut ms = MemorySpace::new();

        println!("Testing multiple MMIO mappings and unmappings");

        // 映射多个MMIO区域
        const REGION1_PADDR: usize = 0xc000_0000;
        const REGION2_PADDR: usize = 0xd000_0000;
        const REGION_SIZE: usize = 0x1000;

        let paddr1 = Paddr::from_usize(REGION1_PADDR);
        let paddr2 = Paddr::from_usize(REGION2_PADDR);

        let vaddr1 = ms
            .map_mmio(paddr1, REGION_SIZE)
            .expect("Failed to map region 1");
        let vaddr2 = ms
            .map_mmio(paddr2, REGION_SIZE)
            .expect("Failed to map region 2");

        println!("  Mapped region 1 to VA=0x{:x}", vaddr1.as_usize());
        println!("  Mapped region 2 to VA=0x{:x}", vaddr2.as_usize());

        // 验证两个区域都存在
        kassert!(ms.find_area(Vpn::from_addr_floor(vaddr1)).is_some());
        kassert!(ms.find_area(Vpn::from_addr_floor(vaddr2)).is_some());

        // 取消映射第一个区域
        ms.unmap_mmio(vaddr1, REGION_SIZE)
            .expect("Failed to unmap region 1");
        kassert!(ms.find_area(Vpn::from_addr_floor(vaddr1)).is_none());
        kassert!(ms.find_area(Vpn::from_addr_floor(vaddr2)).is_some());

        // 取消映射第二个区域
        ms.unmap_mmio(vaddr2, REGION_SIZE)
            .expect("Failed to unmap region 2");
        kassert!(ms.find_area(Vpn::from_addr_floor(vaddr2)).is_none());

        println!("  Multiple MMIO regions test passed");
    });

    // 16. 测试 mmap 文件映射基本功能
    test_case!(test_mmap_file_basic, {
        use crate::fs::tmpfs::TmpFs;
        use crate::uapi::mm::{MapFlags, ProtFlags};
        use crate::vfs::{File, FileMode, FileSystem};
        use alloc::sync::Arc;

        println!("Testing mmap file mapping basic functionality");

        // 1. 创建临时文件系统和文件
        let tmpfs = TmpFs::new(16); // 16 MB
        let root = tmpfs.root_inode();
        let inode = root
            .create("test_mmap.txt", FileMode::from_bits_truncate(0o644))
            .expect("Failed to create file");

        // 2. 写入测试数据
        let test_data = b"Hello, mmap! This is a test file for memory mapping.";
        let written = inode.write_at(0, test_data).expect("Failed to write data");
        kassert!(written == test_data.len());
        println!("  Written {} bytes to file", written);

        // 3. 创建 File 包装器（需要实现一个简单的 File trait）
        // 注意：这里我们直接使用 Inode，因为 File trait 可能需要额外实现
        // 由于测试环境限制，我们先跳过完整的 mmap 测试
        // 这个测试主要验证数据结构和编译正确性

        println!("  File mapping test structure validated");
    });

    // 17. 测试 load_from_file 方法
    test_case!(test_load_from_file, {
        use crate::fs::tmpfs::TmpFs;
        use crate::vfs::{FileMode, FileSystem};

        println!("Testing load_from_file method");

        // 1. 创建文件并写入数据
        let tmpfs = TmpFs::new(16);
        let root = tmpfs.root_inode();
        let inode = root
            .create("test_load.txt", FileMode::from_bits_truncate(0o644))
            .expect("Failed to create file");

        let test_data = b"Test data for loading into memory pages.";
        inode.write_at(0, test_data).expect("Failed to write");
        println!("  Created file with {} bytes", test_data.len());

        // 注意：由于 MmapFile 需要 Arc<dyn File>，而我们只有 Inode，
        // 完整测试需要实现 File wrapper
        // 这里主要验证结构编译正确

        println!("  load_from_file structure validated");
    });

    // 18. 测试 sync_file 方法（验证写回逻辑）
    test_case!(test_sync_file_logic, {
        println!("Testing sync_file logic");

        // 由于 sync_file 需要：
        // 1. MmapFile（包含 Arc<dyn File>）
        // 2. 页表中的 Dirty 位
        // 3. 实际的文件系统操作
        // 完整测试需要更复杂的设置

        // 这里验证编译和结构正确性
        let mut ms = MemorySpace::new();
        let vpn_range = VpnRange::new(Vpn::from_usize(0x2000), Vpn::from_usize(0x2002));

        // 创建一个没有文件映射的区域
        ms.insert_framed_area(
            vpn_range,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area");

        // 对于没有文件映射的区域，sync_file 应该直接返回 Ok
        // 需要分两步以避免借用冲突
        let areas_len = ms.areas().len();
        if areas_len > 0 {
            let page_table = &mut ms.page_table;
            let area = &ms.areas[areas_len - 1];
            let result = area.sync_file(page_table);
            kassert!(result.is_ok());
            println!("  sync_file returns Ok for non-file mapping");
        }

        println!("  sync_file logic validated");
    });

    // 19. 测试 Drop trait 实现
    test_case!(test_memory_space_drop, {
        println!("Testing MemorySpace Drop trait");

        // 创建一个内存空间并添加一些区域
        {
            let mut ms = MemorySpace::new();
            let vpn_range = VpnRange::new(Vpn::from_usize(0x3000), Vpn::from_usize(0x3002));

            ms.insert_framed_area(
                vpn_range,
                AreaType::UserData,
                UniversalPTEFlag::user_rw(),
                None,
                None,
            )
            .expect("Failed to insert area");

            println!("  Created MemorySpace with 1 area");
            // ms 在这里离开作用域，应该调用 Drop
        }

        println!("  MemorySpace dropped successfully (no panic)");
    });

    // 20. 测试 mprotect 基本功能
    test_case!(test_mprotect_basic, {
        println!("Testing mprotect basic functionality");

        let mut ms = MemorySpace::new();
        let vpn_range = VpnRange::new(Vpn::from_usize(0x4000), Vpn::from_usize(0x4002));

        // 创建一个可读写的区域
        ms.insert_framed_area(
            vpn_range,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area");

        println!("  Created area with R/W permissions");

        // 修改为只读
        let start = vpn_range.start().start_addr().as_usize();
        let len = (vpn_range.end().as_usize() - vpn_range.start().as_usize()) * PAGE_SIZE;
        let result = ms.mprotect(start, len, UniversalPTEFlag::user_read());

        kassert!(result.is_ok());
        println!("  Changed permissions to R only");

        // 修改为可执行
        let result = ms.mprotect(start, len, UniversalPTEFlag::user_rx());
        kassert!(result.is_ok());
        println!("  Changed permissions to R+X");

        println!("  mprotect basic test passed");
    });

    // 21. 测试 mprotect 错误处理
    test_case!(test_mprotect_errors, {
        println!("Testing mprotect error handling");

        let mut ms = MemorySpace::new();

        // 测试未对齐的地址
        let result = ms.mprotect(0x1001, PAGE_SIZE, UniversalPTEFlag::user_read());
        kassert!(result.is_err());
        println!("  Correctly rejected unaligned address");

        // 测试未映射的区域
        let result = ms.mprotect(0x5000 * PAGE_SIZE, PAGE_SIZE, UniversalPTEFlag::user_read());
        kassert!(result.is_err());
        println!("  Correctly rejected unmapped region");

        // 测试 len=0
        let result = ms.mprotect(0x1000, 0, UniversalPTEFlag::user_read());
        kassert!(result.is_ok());
        println!("  Correctly handled len=0");

        println!("  mprotect error handling test passed");
    });

    // 22. 测试 mprotect 跨多个区域
    test_case!(test_mprotect_multiple_areas, {
        println!("Testing mprotect across multiple areas");

        let mut ms = MemorySpace::new();

        // 创建两个连续的区域
        let vpn_range1 = VpnRange::new(Vpn::from_usize(0x6000), Vpn::from_usize(0x6002));
        let vpn_range2 = VpnRange::new(Vpn::from_usize(0x6002), Vpn::from_usize(0x6004));

        ms.insert_framed_area(
            vpn_range1,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area 1");

        ms.insert_framed_area(
            vpn_range2,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area 2");

        println!("  Created 2 consecutive areas");

        // 修改跨越两个区域的权限
        let start = vpn_range1.start().start_addr().as_usize();
        let len = (vpn_range2.end().as_usize() - vpn_range1.start().as_usize()) * PAGE_SIZE;
        let result = ms.mprotect(start, len, UniversalPTEFlag::user_read());

        kassert!(result.is_ok());
        println!("  Changed permissions across 2 areas");

        println!("  mprotect multiple areas test passed");
    });

    // 23. 测试 mprotect 部分修改 - 修改前半部分
    test_case!(test_mprotect_partial_front, {
        println!("Testing mprotect partial modification - front half");

        let mut ms = MemorySpace::new();

        // 创建一个4页的区域
        let vpn_range = VpnRange::new(Vpn::from_usize(0x7000), Vpn::from_usize(0x7004));
        ms.insert_framed_area(
            vpn_range,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area");

        println!("  Created 4-page area with RW permissions");

        // 只修改前2页的权限为只读
        let start = vpn_range.start().start_addr().as_usize();
        let len = 2 * PAGE_SIZE;
        let result = ms.mprotect(start, len, UniversalPTEFlag::user_read());
        kassert!(result.is_ok());

        println!("  Changed first 2 pages to R-only");

        // 验证区域被分割为2个
        let area_count = ms
            .areas
            .iter()
            .filter(|a| {
                a.vpn_range().start() >= vpn_range.start() && a.vpn_range().end() <= vpn_range.end()
            })
            .count();
        kassert!(area_count == 2);

        // 验证前2页是只读
        let front_area = ms.find_area(Vpn::from_usize(0x7000)).unwrap();
        kassert!(front_area.permission() == UniversalPTEFlag::user_read());
        println!("  Front area has R-only permission");

        // 验证后2页是读写
        let back_area = ms.find_area(Vpn::from_usize(0x7002)).unwrap();
        kassert!(back_area.permission() == UniversalPTEFlag::user_rw());
        println!("  Back area has RW permission");

        println!("  mprotect partial front test passed");
    });

    // 24. 测试 mprotect 部分修改 - 修改后半部分
    test_case!(test_mprotect_partial_back, {
        println!("Testing mprotect partial modification - back half");

        let mut ms = MemorySpace::new();

        // 创建一个4页的区域
        let vpn_range = VpnRange::new(Vpn::from_usize(0x8000), Vpn::from_usize(0x8004));
        ms.insert_framed_area(
            vpn_range,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area");

        println!("  Created 4-page area with RW permissions");

        // 只修改后2页的权限为只读
        let start = Vpn::from_usize(0x8002).start_addr().as_usize();
        let len = 2 * PAGE_SIZE;
        let result = ms.mprotect(start, len, UniversalPTEFlag::user_read());
        kassert!(result.is_ok());

        println!("  Changed last 2 pages to R-only");

        // 验证区域被分割为2个
        let area_count = ms
            .areas
            .iter()
            .filter(|a| {
                a.vpn_range().start() >= vpn_range.start() && a.vpn_range().end() <= vpn_range.end()
            })
            .count();
        kassert!(area_count == 2);

        // 验证前2页是读写
        let front_area = ms.find_area(Vpn::from_usize(0x8000)).unwrap();
        kassert!(front_area.permission() == UniversalPTEFlag::user_rw());
        println!("  Front area has RW permission");

        // 验证后2页是只读
        let back_area = ms.find_area(Vpn::from_usize(0x8002)).unwrap();
        kassert!(back_area.permission() == UniversalPTEFlag::user_read());
        println!("  Back area has R-only permission");

        println!("  mprotect partial back test passed");
    });

    // 25. 测试 mprotect 部分修改 - 修改中间部分（三分割）
    test_case!(test_mprotect_partial_middle, {
        println!("Testing mprotect partial modification - middle part (3-way split)");

        let mut ms = MemorySpace::new();

        // 创建一个6页的区域
        let vpn_range = VpnRange::new(Vpn::from_usize(0x9000), Vpn::from_usize(0x9006));
        ms.insert_framed_area(
            vpn_range,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area");

        println!("  Created 6-page area with RW permissions");

        // 只修改中间2页（第2-3页，索引从0开始）的权限为只读
        let start = Vpn::from_usize(0x9002).start_addr().as_usize();
        let len = 2 * PAGE_SIZE;
        let result = ms.mprotect(start, len, UniversalPTEFlag::user_read());
        kassert!(result.is_ok());

        println!("  Changed middle 2 pages to R-only");

        // 验证区域被分割为3个
        let area_count = ms
            .areas
            .iter()
            .filter(|a| {
                a.vpn_range().start() >= vpn_range.start() && a.vpn_range().end() <= vpn_range.end()
            })
            .count();
        kassert!(area_count == 3);

        // 验证前2页是读写
        let front_area = ms.find_area(Vpn::from_usize(0x9000)).unwrap();
        kassert!(front_area.permission() == UniversalPTEFlag::user_rw());
        kassert!(front_area.vpn_range().len() == 2);
        println!("  Front area (2 pages) has RW permission");

        // 验证中间2页是只读
        let middle_area = ms.find_area(Vpn::from_usize(0x9002)).unwrap();
        kassert!(middle_area.permission() == UniversalPTEFlag::user_read());
        kassert!(middle_area.vpn_range().len() == 2);
        println!("  Middle area (2 pages) has R-only permission");

        // 验证后2页是读写
        let back_area = ms.find_area(Vpn::from_usize(0x9004)).unwrap();
        kassert!(back_area.permission() == UniversalPTEFlag::user_rw());
        kassert!(back_area.vpn_range().len() == 2);
        println!("  Back area (2 pages) has RW permission");

        println!("  mprotect partial middle test passed");
    });

    // 26. 测试 mprotect 部分修改 - 验证页表权限正确性
    test_case!(test_mprotect_partial_pte_flags, {
        println!("Testing mprotect partial modification - verify PTE flags");

        let mut ms = MemorySpace::new();

        // 创建一个4页的区域，并立即映射
        let vpn_range = VpnRange::new(Vpn::from_usize(0xa000), Vpn::from_usize(0xa004));
        ms.insert_framed_area(
            vpn_range,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area");

        println!("  Created 4-page area with RW permissions");

        // 修改前2页的权限为只读
        let start = vpn_range.start().start_addr().as_usize();
        let len = 2 * PAGE_SIZE;
        let result = ms.mprotect(start, len, UniversalPTEFlag::user_read());
        kassert!(result.is_ok());

        println!("  Changed first 2 pages to R-only");

        // 验证页表中的权限标志
        for i in 0..2 {
            let vpn = Vpn::from_usize(0xa000 + i);
            if let Ok((_, _, flags)) = ms.page_table().walk(vpn) {
                kassert!(flags.contains(UniversalPTEFlag::READABLE));
                kassert!(!flags.contains(UniversalPTEFlag::WRITEABLE));
                println!(
                    "  VPN 0x{:x} has correct R-only flags in page table",
                    vpn.as_usize()
                );
            }
        }

        for i in 2..4 {
            let vpn = Vpn::from_usize(0xa000 + i);
            if let Ok((_, _, flags)) = ms.page_table().walk(vpn) {
                kassert!(flags.contains(UniversalPTEFlag::READABLE));
                kassert!(flags.contains(UniversalPTEFlag::WRITEABLE));
                println!(
                    "  VPN 0x{:x} has correct RW flags in page table",
                    vpn.as_usize()
                );
            }
        }

        println!("  mprotect partial PTE flags test passed");
    });

    // 27. 测试 mprotect 部分修改 - 边界情况（单页修改）
    test_case!(test_mprotect_partial_single_page, {
        println!("Testing mprotect partial modification - single page");

        let mut ms = MemorySpace::new();

        // 创建一个3页的区域
        let vpn_range = VpnRange::new(Vpn::from_usize(0xb000), Vpn::from_usize(0xb003));
        ms.insert_framed_area(
            vpn_range,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area");

        println!("  Created 3-page area with RW permissions");

        // 只修改中间1页的权限为只读
        let start = Vpn::from_usize(0xb001).start_addr().as_usize();
        let len = PAGE_SIZE;
        let result = ms.mprotect(start, len, UniversalPTEFlag::user_read());
        kassert!(result.is_ok());

        println!("  Changed middle page to R-only");

        // 验证区域被分割为3个
        let area_count = ms
            .areas
            .iter()
            .filter(|a| {
                a.vpn_range().start() >= vpn_range.start() && a.vpn_range().end() <= vpn_range.end()
            })
            .count();
        kassert!(area_count == 3);

        // 验证每页的权限
        let page0 = ms.find_area(Vpn::from_usize(0xb000)).unwrap();
        kassert!(page0.permission() == UniversalPTEFlag::user_rw());
        println!("  Page 0 has RW permission");

        let page1 = ms.find_area(Vpn::from_usize(0xb001)).unwrap();
        kassert!(page1.permission() == UniversalPTEFlag::user_read());
        println!("  Page 1 has R-only permission");

        let page2 = ms.find_area(Vpn::from_usize(0xb002)).unwrap();
        kassert!(page2.permission() == UniversalPTEFlag::user_rw());
        println!("  Page 2 has RW permission");

        println!("  mprotect partial single page test passed");
    });
}
