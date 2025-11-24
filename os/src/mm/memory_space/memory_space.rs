use core::cmp::Ordering;

use crate::arch::mm::{paddr_to_vaddr, vaddr_to_paddr};
use crate::config::{MAX_USER_HEAP_SIZE, MEMORY_END, USER_STACK_SIZE, USER_STACK_TOP};
use crate::mm::address::{Paddr, PageNum, Ppn, UsizeConvert, Vaddr, Vpn, VpnRange};
use crate::mm::memory_space::mapping_area::{AreaType, MapType, MappingArea};
use crate::mm::page_table::{ActivePageTableInner, PageTableInner, PagingError, UniversalPTEFlag};
use crate::println;
use crate::sync::SpinLock;
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

    /// 堆顶部 (brk 系统调用使用，仅限用户空间)
    heap_top: Option<Vpn>,
}

impl MemorySpace {
    /// 创建一个新的空内存空间
    pub fn new() -> Self {
        MemorySpace {
            page_table: ActivePageTableInner::new(),
            areas: Vec::new(),
            heap_top: None,
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
    ) -> Result<(), PagingError> {
        let area = MappingArea::new(vpn_range, area_type, MapType::Framed, flags);

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
    ) -> Result<(), PagingError> {
        let area = MappingArea::new(vpn_range, area_type, MapType::Framed, flags);

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
    pub fn from_elf(elf_data: &[u8]) -> Result<(Self, usize, usize), PagingError> {
        use xmas_elf::ElfFile;
        use xmas_elf::program::{SegmentData, Type};

        let elf = ElfFile::new(elf_data).map_err(|_| PagingError::InvalidAddress)?;

        // 检查架构
        if elf.header.pt2.machine().as_machine() != xmas_elf::header::Machine::RISC_V {
            return Err(PagingError::InvalidAddress);
        }

        let mut space = MemorySpace::new();

        // ========== 方案 2：首先映射内核空间 ==========
        // 0. 映射内核空间（所有进程共享相同的内核映射）
        //    - 排除跳板页（将在下面以 U=1 权限映射）
        //    - 所有内核页的 U 标志均为 0，因此用户模式无法访问它们
        space
            .map_kernel_space()
            .expect("Failed to map kernel space for user process");
        // ======================================================

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
                ph.offset() as usize,
            )?;
        }

        // 2. 初始化堆（从 ELF 结束地址开始，页对齐）
        space.heap_top = Some(max_end_vpn);

        // 3. 映射用户栈（带保护页）
        let user_stack_bottom =
            Vpn::from_addr_floor(Vaddr::from_usize(USER_STACK_TOP - USER_STACK_SIZE));
        let user_stack_top = Vpn::from_addr_ceil(Vaddr::from_usize(USER_STACK_TOP));

        space.insert_framed_area(
            VpnRange::new(user_stack_bottom, user_stack_top),
            AreaType::UserStack,
            UniversalPTEFlag::user_rw(),
            None,
        )?;

        let entry_point = elf.header.pt2.entry_point() as usize;

        Ok((space, entry_point, USER_STACK_TOP))
    }

    /// 扩展或收缩堆区域 (brk 系统调用)
    ///
    /// # 错误
    /// - 堆未初始化
    /// - 新的 brk 会超出 MAX_USER_HEAP_SIZE
    /// - 新的 brk 会与现有区域重叠
    pub fn brk(&mut self, new_brk: usize) -> Result<usize, PagingError> {
        let heap_bottom = self.heap_top.ok_or(PagingError::InvalidAddress)?;
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
        let heap_area = self
            .areas
            .iter_mut()
            .find(|a| a.area_type() == AreaType::UserHeap);

        if let Some(area) = heap_area {
            // 存在堆区域，调整大小
            let old_end = area.vpn_range().end();

            match new_end_vpn.cmp(&old_end) {
                Ordering::Greater => {
                    // 扩展
                    let count = new_end_vpn.as_usize() - old_end.as_usize();
                    if count != 0 {
                        area.extend(&mut self.page_table, count)?;
                    }
                }
                Ordering::Less => {
                    // 收缩
                    let count = old_end.as_usize() - new_end_vpn.as_usize();
                    if count != 0 {
                        area.shrink(&mut self.page_table, count)?;
                    }
                }
                Ordering::Equal => { /* 无操作 */ }
            }
        } else {
            // 第一次分配堆，创建新区域
            if new_end_vpn > heap_bottom {
                self.insert_framed_area(
                    VpnRange::new(heap_bottom, new_end_vpn),
                    AreaType::UserHeap,
                    UniversalPTEFlag::user_rw(),
                    None,
                )?;
            }
        }

        Ok(new_brk)
    }

    /// 映射一个匿名区域（简化的 mmap）
    ///
    /// # 参数
    /// - `hint`: 建议的起始地址（0 = 由内核选择）
    /// - `len`: 长度（字节）
    /// - `prot`: 保护标志（PROT_READ | PROT_WRITE | PROT_EXEC）
    pub fn mmap(&mut self, hint: usize, len: usize, prot: usize) -> Result<usize, PagingError> {
        if len == 0 {
            return Err(PagingError::InvalidAddress);
        }

        // 确定起始地址
        let start = if hint == 0 {
            // 内核选择地址：在堆栈顶部之后
            let heap_end = self
                .heap_top
                .ok_or(PagingError::InvalidAddress)?
                .start_addr()
                .as_usize();

            // 查找实际的堆栈末尾
            self.areas
                .iter()
                .filter(|a| a.area_type() == AreaType::UserHeap)
                .map(|a| a.vpn_range().end().start_addr().as_usize())
                .max()
                .unwrap_or(heap_end)
        } else {
            // 用户指定的地址，检查是否可用
            if hint >= USER_STACK_TOP - USER_STACK_SIZE {
                return Err(PagingError::InvalidAddress);
            }
            hint
        };

        let vpn_range = VpnRange::new(
            Vpn::from_addr_floor(Vaddr::from_usize(start)),
            Vpn::from_addr_ceil(Vaddr::from_usize(start + len)),
        );

        // 检查重叠
        for area in &self.areas {
            if area.vpn_range().overlaps(&vpn_range) {
                return Err(PagingError::AlreadyMapped);
            }
        }

        // 转换权限
        let mut flags = UniversalPTEFlag::USER_ACCESSIBLE | UniversalPTEFlag::VALID;
        if prot & 0x1 != 0 {
            flags |= UniversalPTEFlag::READABLE;
        }
        if prot & 0x2 != 0 {
            flags |= UniversalPTEFlag::WRITEABLE;
        }
        if prot & 0x4 != 0 {
            flags |= UniversalPTEFlag::EXECUTABLE;
        }

        self.insert_framed_area(vpn_range, AreaType::UserHeap, flags, None)?;

        Ok(start)
    }

    /// 解除映射一个区域（munmap 系统调用）
    pub fn munmap(&mut self, start: usize, _len: usize) -> Result<(), PagingError> {
        let vpn = Vpn::from_addr_floor(Vaddr::from_usize(start));
        self.remove_area(vpn)
    }

    /// 克隆内存空间（用于 fork 系统调用）
    ///
    /// # 注意
    /// - 直接映射是共享的（不复制）
    /// - 帧映射是深层复制的
    pub fn clone_for_fork(&self) -> Result<Self, PagingError> {
        let mut new_space = MemorySpace::new();
        new_space.heap_top = self.heap_top;

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

    // 6. 测试 MMIO 地址翻译 - 修改为手动映射后测试
    test_case!(test_mmio_translation, {
        use crate::arch::mm::paddr_to_vaddr;
        use crate::mm::memory_space::memory_space::with_kernel_space;

        with_kernel_space(|space| {
            // 获取第一个 MMIO 配置
            if let Some(&(_, mmio_paddr, mmio_size)) = crate::config::MMIO.first() {
                println!(
                    "Testing MMIO translation at PA=0x{:x}, size=0x{:x}",
                    mmio_paddr, mmio_size
                );

                // 手动映射 MMIO 区域
                let paddr = Paddr::from_usize(mmio_paddr);
                let result = space.map_mmio(paddr, mmio_size);
                kassert!(result.is_ok());

                let vaddr = result.unwrap();
                let mmio_vaddr = vaddr.as_usize();
                let vpn = Vpn::from_addr_floor(vaddr);

                println!("  Mapped to VA=0x{:x}", mmio_vaddr);

                // 查找包含该地址的区域
                let area = space.find_area(vpn);
                kassert!(area.is_some());

                if let Some(area) = area {
                    kassert!(area.area_type() == AreaType::KernelMmio);
                    kassert!(area.map_type() == MapType::Direct);
                }

                // 测试页表翻译
                let translated_paddr = space.page_table().translate(Vaddr::from_usize(mmio_vaddr));
                kassert!(translated_paddr.is_some());

                if let Some(paddr) = translated_paddr {
                    println!(
                        "  Translation successful: VA 0x{:x} -> PA 0x{:x}",
                        mmio_vaddr,
                        paddr.as_usize()
                    );
                    // 验证翻译结果（允许页偏移误差）
                    let expected_paddr = mmio_paddr & !0xfff; // 清除页内偏移
                    let actual_paddr = paddr.as_usize() & !0xfff;
                    kassert!(actual_paddr == expected_paddr);
                }
            } else {
                println!("Warning: No MMIO regions configured in platform");
            }
        });
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
}
