use core::cmp::Ordering;

use crate::arch::mm::{paddr_to_vaddr, vaddr_to_paddr};
use crate::config::{MAX_USER_HEAP_SIZE, MEMORY_END, USER_STACK_SIZE, USER_STACK_TOP};
use crate::mm::address::{PageNum, Ppn, UsizeConvert, Vaddr, Vpn, VpnRange};
use crate::mm::memory_space::mapping_area::{AreaType, MapType, MappingArea};
use crate::mm::page_table::{ActivePageTableInner, PageTableInner, PagingError, UniversalPTEFlag};
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

    /// 辅助函数：映射一个 MMIO 区域
    fn map_mmio_region(
        space: &mut MemorySpace,
        addr: usize,
        size: usize,
    ) -> Result<(), PagingError> {
        let vpn_start = Vpn::from_addr_floor(Vaddr::from_usize(addr));
        let vpn_end = Vpn::from_addr_ceil(Vaddr::from_usize(addr + size));

        let mut area = MappingArea::new(
            VpnRange::new(vpn_start, vpn_end),
            AreaType::KernelMmio,
            MapType::Direct,
            UniversalPTEFlag::kernel_rw(),
        );

        area.map(&mut space.page_table)?;
        space.areas.push(area);
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

        for area in &self.areas {
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
}

#[cfg(test)]
mod memory_space_tests {
    use super::*;
    use crate::mm::address::{Vpn, VpnRange};
    use crate::mm::page_table::UniversalPTEFlag;
    use crate::{kassert, test_case};

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
}
