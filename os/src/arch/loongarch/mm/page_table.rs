//! LoongArch64 页表管理
//!
//! 实现 4 级页表结构，使用 `loongArch64` crate 进行 CSR 操作。
//!
//! # 页表结构
//!
//! LoongArch64 使用 4 级页表（48 位虚拟地址）：
//! - Level 3 (Dir4/PGD): 虚拟地址 bits [47:39]，9 位索引
//! - Level 2 (Dir3/PUD): 虚拟地址 bits [38:30]，9 位索引
//! - Level 1 (Dir2/PMD): 虚拟地址 bits [29:21]，9 位索引
//! - Level 0 (Dir1/PT):  虚拟地址 bits [20:12]，9 位索引
//!
//! 每级页表有 512 个条目（2^9），每个条目 8 字节。

use super::PageTableEntry;
use crate::mm::address::{ConvertablePaddr, Paddr, PageNum, Ppn, UsizeConvert, Vaddr, Vpn};
use crate::mm::frame_allocator::{FrameTracker, alloc_frame};
use crate::mm::page_table::{
    PageSize, PageTableEntry as PageTableEntryTrait, PageTableInner as PageTableInnerTrait,
    PagingError, PagingResult, UniversalPTEFlag,
};
use alloc::vec::Vec;

/// 页表内部结构
#[derive(Debug)]
pub struct PageTableInner {
    /// 页表根物理页号
    root_ppn: Ppn,
    /// 存储中间层页表的物理帧，用于自动释放
    frames: Vec<FrameTracker>,
    /// 是否为用户页表
    is_user: bool,
}

impl PageTableInnerTrait<PageTableEntry> for PageTableInner {
    /// LoongArch64 使用 4 级页表
    const LEVELS: usize = 4;
    /// 48 位虚拟地址
    const MAX_VA_BITS: usize = 48;
    /// 48 位物理地址
    const MAX_PA_BITS: usize = 48;

    /// 刷新指定虚拟页号的 TLB 条目
    ///
    /// 使用 INVTLB 指令，op=0x5 表示按虚拟地址刷新
    fn tlb_flush(vpn: Vpn) {
        let vaddr = vpn.start_addr().as_usize();
        unsafe {
            // INVTLB op=0x5: 清除匹配 VA 的 TLB 条目
            // invtlb op, rj, rk
            // op=5: 清除 G=0 且 ASID 匹配且 VA 匹配的条目
            core::arch::asm!(
                "invtlb 0x5, $zero, {0}",
                in(reg) vaddr,
                options(nostack, preserves_flags)
            );
        }
    }

    /// 刷新所有 TLB 条目
    ///
    /// 使用 INVTLB 指令，op=0x0 表示全局刷新
    fn tlb_flush_all() {
        unsafe {
            // INVTLB op=0x0: 清除所有 TLB 条目
            core::arch::asm!(
                "invtlb 0x0, $zero, $zero",
                options(nostack, preserves_flags)
            );
        }
    }

    fn is_user_table(&self) -> bool {
        self.is_user
    }

    /// 激活页表
    ///
    /// 将页表根 PPN 写入 PGDL（低半地址空间）或 PGDH（高半地址空间）
    fn activate(ppn: Ppn) {
        let pgd_paddr = ppn.start_addr().as_usize();
        unsafe {
            // 设置 PGDL (CSR 0x19) - 低半地址空间页全局目录基址
            core::arch::asm!(
                "csrwr {0}, 0x19",
                in(reg) pgd_paddr,
                options(nostack, preserves_flags)
            );
            // 设置 PGDH (CSR 0x1A) - 高半地址空间页全局目录基址
            core::arch::asm!(
                "csrwr {0}, 0x1A",
                in(reg) pgd_paddr,
                options(nostack, preserves_flags)
            );
            // 刷新 TLB
            Self::tlb_flush_all();
        }
    }

    /// 获取当前激活的页表根 PPN
    fn activating_table_ppn() -> Ppn {
        let pgd_paddr: usize;
        unsafe {
            // 读取 PGD (CSR 0x1B) - 根据 BADV 自动选择 PGDL 或 PGDH
            // 这里我们读取 PGDL
            core::arch::asm!(
                "csrrd {0}, 0x19",
                out(reg) pgd_paddr,
                options(nostack, preserves_flags)
            );
        }
        // PGDL 存储的是物理地址，转换为 PPN
        Ppn::from_usize(pgd_paddr >> 12)
    }

    /// 创建新的用户页表
    fn new() -> Self {
        let frame = alloc_frame().expect("Failed to allocate root page table frame");
        let root_ppn = frame.ppn();

        // 清零根页表
        Self::clear_page_table(root_ppn);

        Self {
            root_ppn,
            frames: alloc::vec![frame],
            is_user: true,
        }
    }

    /// 从已有的 PPN 创建页表（不拥有帧所有权）
    fn from_ppn(ppn: Ppn) -> Self {
        Self {
            root_ppn: ppn,
            frames: Vec::new(),
            is_user: false, // from_ppn 通常用于包装现有页表（如内核页表）
        }
    }

    /// 创建新的内核页表
    fn new_as_kernel_table() -> Self {
        let frame = alloc_frame().expect("Failed to allocate kernel page table frame");
        let root_ppn = frame.ppn();

        // 清零根页表
        Self::clear_page_table(root_ppn);

        Self {
            root_ppn,
            frames: alloc::vec![frame],
            is_user: false,
        }
    }

    fn root_ppn(&self) -> Ppn {
        self.root_ppn
    }

    /// 获取指定级别的页表项
    fn get_entry(&self, vpn: Vpn, level: usize) -> Option<(PageTableEntry, PageSize)> {
        if level >= Self::LEVELS {
            return None;
        }

        let mut ppn = self.root_ppn;
        let vpn_value = vpn.as_usize();

        // 从最高级别 (LEVELS-1=3) 遍历到目标级别
        for current_level in (level..Self::LEVELS).rev() {
            let idx = Self::vpn_index(vpn_value, current_level);
            let pte = Self::read_pte(ppn, idx);

            if !pte.is_valid() {
                return None;
            }

            if current_level == level {
                let page_size = PageSize::Size4K; // 当前仅支持 4K 页
                return Some((pte, page_size));
            }

            ppn = pte.ppn();
        }

        None
    }

    /// 虚拟地址转物理地址
    fn translate(&self, vaddr: Vaddr) -> Option<Paddr> {
        let vpn = Vpn::from_addr_floor(vaddr);
        let offset = vaddr.as_usize() & 0xfff; // 页内偏移

        match self.walk(vpn) {
            Ok((ppn, page_size, _flags)) => {
                let paddr_base = match page_size {
                    PageSize::Size4K => ppn.start_addr().as_usize(),
                    _ => ppn.start_addr().as_usize(), // 暂时按 4K 处理
                };
                Some(Paddr::from_usize(paddr_base + offset))
            }
            Err(_) => None,
        }
    }

    /// 建立虚拟页到物理页的映射
    fn map(
        &mut self,
        vpn: Vpn,
        ppn: Ppn,
        _page_size: PageSize,
        flags: UniversalPTEFlag,
    ) -> PagingResult<()> {
        // 验证标志位：叶子节点必须至少设置可读或可执行
        if !flags.intersects(
            UniversalPTEFlag::READABLE | UniversalPTEFlag::WRITEABLE | UniversalPTEFlag::EXECUTABLE,
        ) {
            return Err(PagingError::InvalidFlags);
        }

        // 当前仅支持 4K 页（level 0）
        let target_level = 0;

        let mut current_ppn = self.root_ppn;
        let vpn_value = vpn.as_usize();

        // 从最高级别 (3) 遍历到目标级别
        for level in (target_level..Self::LEVELS).rev() {
            let idx = Self::vpn_index(vpn_value, level);
            let pte = Self::read_pte(current_ppn, idx);

            if level == target_level {
                // 已到达目标级别，创建叶子节点
                if pte.is_valid() {
                    return Err(PagingError::AlreadyMapped);
                }

                let new_pte = PageTableEntry::new_leaf(ppn, flags | UniversalPTEFlag::VALID);
                Self::write_pte(current_ppn, idx, new_pte);
                Self::tlb_flush(vpn);
                return Ok(());
            } else {
                // 中间级别
                if !pte.is_valid() {
                    // 分配新的页表
                    let new_frame = alloc_frame().ok_or(PagingError::FrameAllocFailed)?;
                    let new_ppn = new_frame.ppn();

                    // 清零新页表
                    Self::clear_page_table(new_ppn);

                    // 创建表节点
                    let table_pte = PageTableEntry::new_table(new_ppn);
                    Self::write_pte(current_ppn, idx, table_pte);
                    self.frames.push(new_frame);

                    current_ppn = new_ppn;
                } else if pte.is_huge() {
                    return Err(PagingError::HugePageConflict);
                } else {
                    current_ppn = pte.ppn();
                }
            }
        }

        Err(PagingError::InvalidAddress)
    }

    /// 解除虚拟页的映射
    fn unmap(&mut self, vpn: Vpn) -> PagingResult<()> {
        let mut current_ppn = self.root_ppn;
        let vpn_value = vpn.as_usize();

        for level in (0..Self::LEVELS).rev() {
            let idx = Self::vpn_index(vpn_value, level);
            let pte = Self::read_pte(current_ppn, idx);

            if !pte.is_valid() {
                return Err(PagingError::NotMapped);
            }

            if pte.is_huge() || level == 0 {
                // 找到叶子节点，清除映射
                Self::write_pte(current_ppn, idx, PageTableEntry::empty());
                Self::tlb_flush(vpn);
                return Ok(());
            }

            current_ppn = pte.ppn();
        }

        Err(PagingError::NotMapped)
    }

    /// 移动映射
    fn mvmap(
        &mut self,
        vpn: Vpn,
        target_ppn: Ppn,
        page_size: PageSize,
        flags: UniversalPTEFlag,
    ) -> PagingResult<()> {
        self.unmap(vpn)?;
        self.map(vpn, target_ppn, page_size, flags)
    }

    /// 更新页表项标志位
    fn update_flags(&mut self, vpn: Vpn, flags: UniversalPTEFlag) -> PagingResult<()> {
        let mut current_ppn = self.root_ppn;
        let vpn_value = vpn.as_usize();

        for level in (0..Self::LEVELS).rev() {
            let idx = Self::vpn_index(vpn_value, level);
            let mut pte = Self::read_pte(current_ppn, idx);

            if !pte.is_valid() {
                return Err(PagingError::NotMapped);
            }

            if pte.is_huge() || level == 0 {
                pte.set_flags(flags | UniversalPTEFlag::VALID);
                Self::write_pte(current_ppn, idx, pte);
                Self::tlb_flush(vpn);
                return Ok(());
            }

            current_ppn = pte.ppn();
        }

        Err(PagingError::NotMapped)
    }

    /// 遍历页表，获取映射信息
    fn walk(&self, vpn: Vpn) -> PagingResult<(Ppn, PageSize, UniversalPTEFlag)> {
        let mut ppn = self.root_ppn;
        let vpn_value = vpn.as_usize();

        for level in (0..Self::LEVELS).rev() {
            let idx = Self::vpn_index(vpn_value, level);
            let pte = Self::read_pte(ppn, idx);

            if !pte.is_valid() {
                return Err(PagingError::NotMapped);
            }

            if pte.is_huge() || level == 0 {
                let page_size = PageSize::Size4K; // 当前仅支持 4K
                return Ok((pte.ppn(), page_size, pte.flags()));
            }

            ppn = pte.ppn();
        }

        Err(PagingError::NotMapped)
    }
}

impl PageTableInner {
    /// 从 VPN 计算指定级别的索引
    ///
    /// 每级 9 位索引：
    /// - Level 3: bits [35:27] of VPN (对应 VA bits [47:39])
    /// - Level 2: bits [26:18] of VPN (对应 VA bits [38:30])
    /// - Level 1: bits [17:9] of VPN  (对应 VA bits [29:21])
    /// - Level 0: bits [8:0] of VPN   (对应 VA bits [20:12])
    #[inline]
    fn vpn_index(vpn: usize, level: usize) -> usize {
        (vpn >> (9 * level)) & 0x1ff
    }

    /// 读取页表项
    #[inline]
    fn read_pte(ppn: Ppn, index: usize) -> PageTableEntry {
        let pte_array = unsafe {
            core::slice::from_raw_parts(
                ppn.start_addr().to_vaddr().as_usize() as *const PageTableEntry,
                512,
            )
        };
        pte_array[index]
    }

    /// 写入页表项
    #[inline]
    fn write_pte(ppn: Ppn, index: usize, pte: PageTableEntry) {
        let pte_array = unsafe {
            core::slice::from_raw_parts_mut(
                ppn.start_addr().to_vaddr().as_usize() as *mut PageTableEntry,
                512,
            )
        };
        pte_array[index] = pte;
    }

    /// 清零页表
    fn clear_page_table(ppn: Ppn) {
        let pte_array = unsafe {
            core::slice::from_raw_parts_mut(
                ppn.start_addr().to_vaddr().as_usize() as *mut PageTableEntry,
                512,
            )
        };
        for entry in pte_array.iter_mut() {
            *entry = PageTableEntry::empty();
        }
    }
}

// 单元测试模块
#[cfg(test)]
mod page_table_tests {
    use super::*;
    use crate::mm::page_table::PageTableInner as PageTableInnerTrait;
    use crate::{kassert, test_case};

    // 1. 页表创建测试
    test_case!(test_pt_create, {
        let pt = PageTableInner::new();
        // 根 PPN 应该有效 (大于 0)
        kassert!(pt.root_ppn().as_usize() > 0);
        // 默认创建为用户页表
        kassert!(pt.is_user_table());
    });

    // 2. 映射与转换测试
    test_case!(test_pt_map_translate, {
        let mut pt = PageTableInner::new();
        let vpn = Vpn::from_usize(0x1000);
        let ppn = Ppn::from_usize(0x80000);

        // 映射 vpn -> ppn
        let result = pt.map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::kernel_rw());
        kassert!(result.is_ok());

        // 转换验证 - 使用 vpn.start_addr() 获取正确的虚拟地址
        let vaddr = vpn.start_addr();
        let translated = pt.translate(vaddr);
        kassert!(translated.is_some());
        let paddr = translated.unwrap();
        // 验证转换后的物理页号是否正确
        kassert!(paddr.as_usize() >> 12 == ppn.as_usize());
    });

    // 3. 解除映射测试
    test_case!(test_pt_unmap, {
        let mut pt = PageTableInner::new();
        let vpn = Vpn::from_usize(0x1000);
        let ppn = Ppn::from_usize(0x80000);

        // 先映射
        pt.map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::kernel_rw())
            .unwrap();

        // 解除映射
        let result = pt.unmap(vpn);
        kassert!(result.is_ok());

        // 应该不再被映射
        let vaddr = vpn.start_addr();
        let translated = pt.translate(vaddr);
        kassert!(translated.is_none());
    });

    // 4. 错误测试：已映射
    test_case!(test_pt_error_already_mapped, {
        let mut pt = PageTableInner::new();
        let vpn = Vpn::from_usize(0x1000);

        // 第一次映射成功
        let result1 = pt.map(
            vpn,
            Ppn::from_usize(0x80000),
            PageSize::Size4K,
            UniversalPTEFlag::kernel_rw(),
        );
        kassert!(result1.is_ok());

        // 第二次映射应该失败 (返回 AlreadyMapped 错误)
        let result2 = pt.map(
            vpn,
            Ppn::from_usize(0x80001),
            PageSize::Size4K,
            UniversalPTEFlag::kernel_rw(),
        );
        kassert!(result2.is_err());
    });

    // 5. 页表遍历 (Walk) 测试
    test_case!(test_pt_walk, {
        let mut pt = PageTableInner::new();
        let vpn = Vpn::from_usize(0x1000);
        let ppn = Ppn::from_usize(0x80000);
        let original_flags = UniversalPTEFlag::kernel_rw();

        // 先映射
        pt.map(vpn, ppn, PageSize::Size4K, original_flags).unwrap();

        // 遍历获取映射信息
        let walk_result = pt.walk(vpn);
        kassert!(walk_result.is_ok());

        let (mapped_ppn, _, mapped_flags) = walk_result.unwrap();
        kassert!(mapped_ppn == ppn);

        // 注意：LoongArch 的 D 位同时表示可写和脏位
        // 因此 kernel_rw() 经过 from_universal -> to_universal 后会多出 DIRTY 标志
        // 验证关键权限位正确即可
        kassert!(mapped_flags.contains(UniversalPTEFlag::VALID));
        kassert!(mapped_flags.contains(UniversalPTEFlag::READABLE));
        kassert!(mapped_flags.contains(UniversalPTEFlag::WRITEABLE));
    });

    // 6. 更新标志位测试
    test_case!(test_pt_update_flags, {
        let mut pt = PageTableInner::new();
        let vpn = Vpn::from_usize(0x1000);
        let ppn = Ppn::from_usize(0x80000);

        // 初始映射为 kernel_rw
        pt.map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::kernel_rw())
            .unwrap();

        // 更新为内核只读 (kernel_r = VALID | READABLE)
        let update_flags = UniversalPTEFlag::kernel_r();
        let result = pt.update_flags(vpn, update_flags);
        kassert!(result.is_ok());

        // 验证标志位是否已更改为只读
        let (_, _, flags) = pt.walk(vpn).unwrap();
        kassert!(flags.contains(UniversalPTEFlag::VALID));
        kassert!(flags.contains(UniversalPTEFlag::READABLE));
        // kernel_r 不应包含 WRITEABLE
        kassert!(!flags.contains(UniversalPTEFlag::WRITEABLE));
    });

    // 7. 多重映射测试
    test_case!(test_pt_multiple_mappings, {
        let mut pt = PageTableInner::new();

        // 映射多个 VPN
        for i in 0..10 {
            let vpn = Vpn::from_usize(0x1000 + i);
            let ppn = Ppn::from_usize(0x80000 + i);
            let result = pt.map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::kernel_rw());
            kassert!(result.is_ok());
        }

        // 验证所有映射
        for i in 0..10 {
            let vpn = Vpn::from_usize(0x1000 + i);
            let expected_ppn = Ppn::from_usize(0x80000 + i);
            let (mapped_ppn, _, _) = pt.walk(vpn).unwrap();
            kassert!(mapped_ppn == expected_ppn);
        }
    });
}
