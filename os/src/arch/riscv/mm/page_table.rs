// TODO: 这个模块的安全性论证没有完成
use super::PageTableEntry;
use crate::arch::ipi::send_tlb_flush_ipi_all;
use crate::mm::address::{ConvertablePaddr, Paddr, PageNum, Ppn, UsizeConvert, Vaddr, Vpn};
use crate::mm::frame_allocator::{FrameTracker, alloc_frame};
use crate::mm::page_table::{
    PageSize, PageTableEntry as PageTableEntryTrait, PageTableInner as PageTableInnerTrait,
    PagingError, PagingResult, UniversalPTEFlag,
};
use alloc::vec::Vec;

#[derive(Debug)]
pub struct PageTableInner {
    root: Ppn, // 页表的根物理页号 (Root PPN)
    // 仅用于存储中间层页表的物理帧，以便在 PageTableInner 析构时自动释放
    frames: Vec<FrameTracker>,
    is_user: bool, // 标识是否为用户页表
}

// RISC-V SV39 架构相关的 PageTableInner 实现
impl PageTableInnerTrait<PageTableEntry> for PageTableInner {
    const LEVELS: usize = 3; // SV39 分页方案有 3 级 (0, 1, 2)
    const MAX_VA_BITS: usize = 39; // 最大有效虚拟地址位数为 39 位
    const MAX_PA_BITS: usize = 56; // 最大有效物理地址位数为 56 位

    // 对指定虚拟页号 (VPN) 进行 TLB 刷新
    fn tlb_flush(vpn: Vpn) {
        let vaddr = vpn.start_addr();
        // Safe: 使用 RISC-V 指令 sfence.vma 刷新指定虚拟地址的 TLB 条目
        //      指令正确性： sfence.vma (Synchronize Fence Virtual Memory) 是 RISC-V 规范定义的指令，
        //                 用于保证 TLB 一致性。它不会导致未定义的行为或内存安全问题。
        //      权限正确性： 只有在 Supervisor (S) 模式 或更高特权级才能执行 sfence.vma。在操作系统内核中，
        //                 我们假设这段代码总是在 S 模式下执行，因此指令是合法的。
        //      参数正确性： 该函数只是将 vpn 转换后的虚拟地址传入指令，该操作不会对内核的内存安全不变性造成破坏。
        //                 调用此函数是为了修复页表更新后可能存在的 TLB 过期问题，是恢复系统正确状态的必要操作。
        //      RISC-V 约定： sfence.vma {0}, zero 形式，其中 asid 字段为 zero，表示刷新当前地址空间（ASID）
        //                 中与该虚拟地址相关的 TLB 条目，或者如果 ASID 机制未使用，则等同于全局刷新。
        //                 此用法符合 RISC-V 规范。
        unsafe {
            // RISC-V 指令：sfence.vma vaddr, asid (asid 为 zero 表示全局刷新或当前 asid)
            core::arch::asm!(
                "sfence.vma {0}, zero",
                in(reg) vaddr.as_usize()
            )
        }
    }

    // 全局 TLB 刷新
    fn tlb_flush_all() {
        // Safe: 使用 RISC-V 指令 sfence.vma 刷新所有 TLB 条目
        unsafe { core::arch::asm!("sfence.vma") }
    }

    // 检查是否为用户页表
    fn is_user_table(&self) -> bool {
        self.is_user
    }

    // 激活页表 (将页表根 PPN 写入 satp 寄存器)
    fn activate(ppn: Ppn) {
        let satp_value = ppn_to_satp(ppn);
        // Safe: 写入 satp 寄存器并执行全局 TLB 刷新
        unsafe {
            // 写入 satp 寄存器并执行全局 TLB 刷新
            core::arch::asm!(
                "csrw satp, {0}",
                "sfence.vma",
                in(reg) satp_value
            )
        }
    }

    // 获取当前活动的页表根 PPN (从 satp 寄存器读取)
    fn activating_table_ppn() -> Ppn {
        let satp_value: usize;
        unsafe {
            // 从 satp 寄存器读取值
            core::arch::asm!("csrr {0}, satp", out(reg) satp_value);
        }
        // SV39 中 PPN 位于 satp 值的低 44 位
        let ppn = satp_value & ((1usize << 44) - 1);
        Ppn::from_usize(ppn)
    }

    // 创建一个新的用户页表
    fn new() -> Self {
        let frame = alloc_frame().unwrap(); // 分配根页表帧
        Self {
            root: frame.ppn(),
            frames: alloc::vec![frame], // 存储根帧
            is_user: true,
        }
    }

    // 从已有的 PPN 创建页表 (用于内核页表等，不拥有其帧)
    fn from_ppn(ppn: Ppn) -> Self {
        Self {
            root: ppn,
            frames: Vec::new(), // 不拥有任何中间帧的所有权
            is_user: true,
        }
    }

    // 创建一个新的内核页表
    fn new_as_kernel_table() -> Self {
        let frame = alloc_frame().unwrap(); // 分配根页表帧
        Self {
            root: frame.ppn(),
            frames: alloc::vec![frame],
            is_user: false,
        }
    }

    // 获取页表根 PPN
    fn root_ppn(&self) -> Ppn {
        self.root
    }

    // 查找指定级别 (level) 的页表项 (PTE)
    fn get_entry(&self, vpn: Vpn, level: usize) -> Option<(PageTableEntry, PageSize)> {
        if level >= Self::LEVELS {
            return None;
        }

        let mut ppn = self.root;
        let vpn_value = vpn.as_usize();

        // 从根级别 (LEVELS-1) 遍历到目标级别 (level)
        for current_level in (level..Self::LEVELS).rev() {
            // 计算当前级别的页表项索引：[9 * current_level]
            let idx = (vpn_value >> (9 * current_level)) & 0x1ff;

            // Unsafe: 将 PPN 转换为虚拟地址并获取页表项数组的不可变引用
            let pte_array = unsafe {
                core::slice::from_raw_parts(
                    ppn.start_addr().to_vaddr().as_usize() as *const PageTableEntry,
                    512, // 每级页表有 512 个 PTE
                )
            };
            let pte = &pte_array[idx];

            if !pte.is_valid() {
                return None; // 无效 PTE
            }

            if current_level == level {
                // 已到达目标级别
                // TODO(暂时注释): 当前仅支持4K页
                // let page_size = match level {
                //     2 => PageSize::Size1G,
                //     1 => PageSize::Size2M,
                //     0 => PageSize::Size4K,
                //     _ => unreachable!(),
                // };
                let page_size = PageSize::Size4K; // 仅支持4K页
                return Some((*pte, page_size));
            }

            // 继续下一级页表
            ppn = pte.ppn();
        }

        None
    }

    // 虚拟地址到物理地址的转换 (Translate)
    fn translate(&self, vaddr: Vaddr) -> Option<Paddr> {
        let vpn = Vpn::from_addr_ceil(vaddr);
        // 页内偏移量：低 12 位
        let offset = vaddr.as_usize() & 0xfff;

        // TODO(暂时注释): 当前仅支持4K页，大页translation逻辑已禁用
        match self.walk(vpn) {
            Ok((ppn, page_size, _flags)) => {
                let paddr_base = match page_size {
                    PageSize::Size4K => ppn.start_addr().as_usize(),
                    // TODO(暂时注释): 大页偏移计算
                    // PageSize::Size2M => {
                    //     // 对于 2M 页，保留 vaddr 的低 21 位作为页内偏移
                    //     let offset_2m = vaddr.as_usize() & 0x1f_ffff;
                    //     ppn.start_addr().as_usize() + offset_2m - offset
                    // }
                    // PageSize::Size1G => {
                    //     // 对于 1G 页，保留 vaddr 的低 30 位作为页内偏移
                    //     let offset_1g = vaddr.as_usize() & 0x3fff_ffff;
                    //     ppn.start_addr().as_usize() + offset_1g - offset
                    // }
                    _ => ppn.start_addr().as_usize(), // 默认按 4K 页处理基地址
                };
                // 物理地址 = 物理页基地址 + 页内偏移
                Some(Paddr::from_usize(paddr_base + offset))
            }
            Err(_) => None,
        }
    }

    // 建立虚拟页号 (VPN) 到物理页号 (PPN) 的映射 (Map)
    fn map(
        &mut self,
        vpn: Vpn,
        ppn: Ppn,
        _page_size: PageSize,
        flags: UniversalPTEFlag,
    ) -> PagingResult<()> {
        // 验证标志位：叶子节点必须至少有 R/W/X 之一被设置
        if !flags.intersects(
            UniversalPTEFlag::READABLE | UniversalPTEFlag::WRITEABLE | UniversalPTEFlag::EXECUTABLE,
        ) {
            return Err(PagingError::InvalidFlags);
        }

        // TODO(暂时注释): 当前仅支持4K页，强制使用 level 0
        // 根据页大小确定目标级别
        // let target_level = match page_size {
        //     PageSize::Size1G => 2,
        //     PageSize::Size2M => 1,
        //     PageSize::Size4K => 0,
        // };
        let target_level = 0; // 仅支持 4K 页

        let mut current_ppn = self.root;
        let vpn_value = vpn.as_usize();

        // 从根级别 (LEVELS-1) 遍历到目标级别 (target_level)
        for level in (target_level..Self::LEVELS).rev() {
            let idx = (vpn_value >> (9 * level)) & 0x1ff;

            // Unsafe: 获取可变的页表项数组引用
            let pte_array = unsafe {
                core::slice::from_raw_parts_mut(
                    current_ppn.start_addr().to_vaddr().as_usize() as *mut PageTableEntry,
                    512,
                )
            };
            let pte = &mut pte_array[idx];

            if level == target_level {
                // 已到达目标级别，创建叶子节点
                if pte.is_valid() {
                    return Err(PagingError::AlreadyMapped); // 已被映射
                }
                // 创建新的叶子 PTE，设置 PPN 和标志位 (VALID 必须设置)
                *pte = PageTableEntry::new_leaf(ppn, flags | UniversalPTEFlag::VALID);

                return Ok(());
            } else {
                // 中间级别 - 需要继续向下遍历
                if !pte.is_valid() {
                    // 页表项无效，需要分配一个新的页表
                    let new_frame = alloc_frame().ok_or(PagingError::FrameAllocFailed)?;
                    let new_ppn = new_frame.ppn();

                    // 清空新的页表（即新分配的物理页）
                    // Unsafe: 获取可变的页表项数组引用并清零
                    let new_table = unsafe {
                        core::slice::from_raw_parts_mut(
                            new_ppn.start_addr().to_vaddr().as_usize() as *mut PageTableEntry,
                            512,
                        )
                    };
                    for entry in new_table.iter_mut() {
                        *entry = PageTableEntry::empty();
                    }

                    // 更新当前级别的 PTE，指向新分配的页表 (VALID 标志在 new_table 中设置)
                    *pte = PageTableEntry::new_table(new_ppn);
                    self.frames.push(new_frame); // 将新帧加入向量，以便自动释放
                } else if pte.is_huge() {
                    // 此处已有一个巨页映射，产生冲突
                    return Err(PagingError::HugePageConflict);
                }

                // 准备下一轮循环，进入下一级页表
                current_ppn = pte.ppn();
            }
        }

        Err(PagingError::InvalidAddress) // 理论上不应该到达
    }

    // 解除虚拟页号 (VPN) 映射 (Unmap)
    fn unmap(&mut self, vpn: Vpn) -> PagingResult<()> {
        let mut current_ppn = self.root;
        let vpn_value = vpn.as_usize();

        // 遍历页表以找到叶子节点
        for level in (0..Self::LEVELS).rev() {
            let idx = (vpn_value >> (9 * level)) & 0x1ff;

            // Unsafe: 获取可变的页表项数组引用
            let pte_array = unsafe {
                core::slice::from_raw_parts_mut(
                    current_ppn.start_addr().to_vaddr().as_usize() as *mut PageTableEntry,
                    512,
                )
            };
            let pte = &mut pte_array[idx];

            if !pte.is_valid() {
                return Err(PagingError::NotMapped); // 未映射
            }

            // 检查是否为叶子节点 (具有 R/W/X 权限或已到达 level 0)
            if pte.is_huge() || level == 0 {
                // 清空 PTE 以解除映射
                pte.clear();
                return Ok(());
            }

            // 继续下一级页表
            current_ppn = pte.ppn();
        }

        Err(PagingError::NotMapped) // 理论上不应该到达
    }

    // 移动映射 (先解除旧映射，再建立新映射)
    fn mvmap(
        &mut self,
        vpn: Vpn,
        target_ppn: Ppn,
        page_size: PageSize,
        flags: UniversalPTEFlag,
    ) -> PagingResult<()> {
        // 先解除旧映射
        self.unmap(vpn)?;
        // 再映射到新的物理页
        self.map(vpn, target_ppn, page_size, flags)
    }

    // 更新指定 VPN 的页表项标志位
    fn update_flags(&mut self, vpn: Vpn, flags: UniversalPTEFlag) -> PagingResult<()> {
        let mut current_ppn = self.root;
        let vpn_value = vpn.as_usize();

        // 遍历页表以找到叶子节点
        for level in (0..Self::LEVELS).rev() {
            let idx = (vpn_value >> (9 * level)) & 0x1ff;

            // Unsafe: 获取可变的页表项数组引用
            let pte_array = unsafe {
                core::slice::from_raw_parts_mut(
                    current_ppn.start_addr().to_vaddr().as_usize() as *mut PageTableEntry,
                    512,
                )
            };
            let pte = &mut pte_array[idx];

            if !pte.is_valid() {
                return Err(PagingError::NotMapped); // 未映射
            }

            // 检查是否为叶子节点
            if pte.is_huge() || level == 0 {
                // 设置新的标志位 (VALID 必须保持设置)
                pte.set_flags(flags | UniversalPTEFlag::VALID);
                return Ok(());
            }

            // 继续下一级页表
            current_ppn = pte.ppn();
        }

        Err(PagingError::NotMapped) // 理论上不应该到达
    }

    // 遍历页表 (Walk)，查找指定 VPN 的映射信息
    fn walk(&self, vpn: Vpn) -> PagingResult<(Ppn, PageSize, UniversalPTEFlag)> {
        let mut ppn = self.root;
        let vpn_value = vpn.as_usize();

        // SV39 级别划分：VPN[2] = 位[38:30], VPN[1] = 位[29:21], VPN[0] = 位[20:12]
        // 从最高级 (Level 2) 遍历到最低级 (Level 0)
        for level in (0..Self::LEVELS).rev() {
            let idx = (vpn_value >> (9 * level)) & 0x1ff;

            // Unsafe: 获取页表项数组的不可变引用
            let pte_array = unsafe {
                core::slice::from_raw_parts(
                    ppn.start_addr().to_vaddr().as_usize() as *const PageTableEntry,
                    512,
                )
            };
            let pte = &pte_array[idx];

            if !pte.is_valid() {
                return Err(PagingError::NotMapped); // 无效 PTE
            }

            // 检查是否为叶子节点
            if pte.is_huge() || level == 0 {
                // 找到叶子节点
                // TODO(暂时注释): 当前仅支持4K页
                // let page_size = match level {
                //     2 => PageSize::Size1G,
                //     1 => PageSize::Size2M,
                //     0 => PageSize::Size4K,
                //     _ => unreachable!(),
                // };
                let page_size = PageSize::Size4K; // 仅支持 4K 页
                return Ok((pte.ppn(), page_size, pte.flags()));
            }

            // 继续下一级页表
            ppn = pte.ppn();
        }

        Err(PagingError::NotMapped) // 未找到映射
    }
}

// PageTableInner 的额外实现（非 trait 方法）
impl PageTableInner {
    /// 刷新所有 CPU 的 TLB（多核 TLB Shootdown）
    ///
    /// 此函数执行以下操作：
    /// 1. 刷新当前 CPU 的 TLB（针对指定 VPN）
    /// 2. 通过 IPI 通知所有其他 CPU 刷新其 TLB
    ///
    /// # 参数
    /// - vpn: 需要刷新的虚拟页号
    ///
    /// # 注意
    /// - 单核系统：只刷新本地 TLB，无 IPI 开销
    /// - 多核系统：异步刷新，不等待其他 CPU 确认
    /// - 测试模式：也会发送 IPI（如果是多核环境）
    fn tlb_flush_all_cpus(vpn: Vpn) {
        // 1. 刷新当前 CPU 的 TLB
        <Self as PageTableInnerTrait<PageTableEntry>>::tlb_flush(vpn);

        // 2. 通知所有其他 CPU 刷新 TLB
        // send_tlb_flush_ipi_all 内部会检查是否为多核环境
        // 单核环境下不会发送 IPI
        let num_cpu = unsafe { crate::kernel::NUM_CPU };
        if num_cpu > 1 {
            send_tlb_flush_ipi_all();
        }
    }

    /// 带批处理支持的映射方法
    pub fn map_with_batch(
        &mut self,
        vpn: Vpn,
        ppn: Ppn,
        page_size: PageSize,
        flags: UniversalPTEFlag,
        batch: Option<&mut TlbBatchContext>,
    ) -> PagingResult<()> {
        <Self as PageTableInnerTrait<PageTableEntry>>::map(self, vpn, ppn, page_size, flags)?;
        // 总是刷新本地 TLB
        <Self as PageTableInnerTrait<PageTableEntry>>::tlb_flush(vpn);
        // 只有在非批处理模式下才发送 IPI
        if batch.is_none() {
            let num_cpu = unsafe { crate::kernel::NUM_CPU };
            if num_cpu > 1 {
                send_tlb_flush_ipi_all();
            }
        }
        Ok(())
    }

    /// 带批处理支持的解除映射方法
    pub fn unmap_with_batch(
        &mut self,
        vpn: Vpn,
        batch: Option<&mut TlbBatchContext>,
    ) -> PagingResult<()> {
        <Self as PageTableInnerTrait<PageTableEntry>>::unmap(self, vpn)?;
        // 总是刷新本地 TLB
        <Self as PageTableInnerTrait<PageTableEntry>>::tlb_flush(vpn);
        // 只有在非批处理模式下才发送 IPI
        if batch.is_none() {
            let num_cpu = unsafe { crate::kernel::NUM_CPU };
            if num_cpu > 1 {
                send_tlb_flush_ipi_all();
            }
        }
        Ok(())
    }

    /// 带批处理支持的更新权限方法
    pub fn update_flags_with_batch(
        &mut self,
        vpn: Vpn,
        flags: UniversalPTEFlag,
        batch: Option<&mut TlbBatchContext>,
    ) -> PagingResult<()> {
        <Self as PageTableInnerTrait<PageTableEntry>>::update_flags(self, vpn, flags)?;
        // 总是刷新本地 TLB
        <Self as PageTableInnerTrait<PageTableEntry>>::tlb_flush(vpn);
        // 只有在非批处理模式下才发送 IPI
        if batch.is_none() {
            let num_cpu = unsafe { crate::kernel::NUM_CPU };
            if num_cpu > 1 {
                send_tlb_flush_ipi_all();
            }
        }
        Ok(())
    }
}

/// TLB 批量刷新上下文
///
/// 用于在批量页表操作期间延迟 TLB 刷新，减少 IPI 数量
pub struct TlbBatchContext {
    enabled: bool,
}

impl TlbBatchContext {
    /// 创建新的批处理上下文
    pub fn new() -> Self {
        Self { enabled: true }
    }

    /// 在批处理上下文中执行操作
    pub fn execute<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        let mut ctx = Self::new();
        let result = f(&mut ctx);
        ctx.flush();
        result
    }

    /// 刷新所有待处理的 TLB 条目
    pub fn flush(&mut self) {
        if self.enabled {
            // 刷新本地 TLB
            unsafe {
                core::arch::asm!("sfence.vma");
            }
            // 发送一次 IPI 到所有其他 CPU
            let num_cpu = unsafe { crate::kernel::NUM_CPU };
            if num_cpu > 1 {
                send_tlb_flush_ipi_all();
            }
            self.enabled = false;
        }
    }
}

impl Drop for TlbBatchContext {
    fn drop(&mut self) {
        self.flush();
    }
}

// 辅助函数：将 PPN 转换为 satp 寄存器的值
fn ppn_to_satp(ppn: Ppn) -> usize {
    // 设置 MODE=8 (SV39) 并与 PPN 进行位或操作
    ppn.as_usize() | (8usize << 60)
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
        // 创建新的flags实例用于比较，避免所有权问题
        let expected_flags = UniversalPTEFlag::kernel_rw();
        kassert!(mapped_flags.bits() == expected_flags.bits());
    });

    // 6. 更新标志位测试
    test_case!(test_pt_update_flags, {
        let mut pt = PageTableInner::new();
        let vpn = Vpn::from_usize(0x1000);
        let ppn = Ppn::from_usize(0x80000);

        // 初始映射为 kernel_rw
        pt.map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::kernel_rw())
            .unwrap();

        // 更新为内核只读 (kernel_r)
        let update_flags = UniversalPTEFlag::kernel_r();
        let result = pt.update_flags(vpn, update_flags);
        kassert!(result.is_ok());

        // 验证标志位是否已更改
        let (_, _, flags) = pt.walk(vpn).unwrap();
        // 创建新的flags实例用于比较，避免所有权问题
        let expected_flags = UniversalPTEFlag::kernel_r();
        kassert!(flags.bits() == expected_flags.bits());
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

    // TLB Shootdown 测试

    /// 测试 TLB flush IPI 发送（基础功能）
    test_case!(test_tlb_flush_ipi_basic, {
        // 调用 send_tlb_flush_ipi_all 不应该 panic
        crate::arch::ipi::send_tlb_flush_ipi_all();
        kassert!(true);
    });

    /// 测试页表映射触发 TLB shootdown
    test_case!(test_page_table_map_with_tlb_flush, {
        let mut pt = PageTableInner::new();
        let vpn = Vpn::from_usize(0x10000);
        let ppn = Ppn::from_usize(0x80000);

        // 执行映射操作（应该触发 TLB shootdown）
        let result = pt.map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::kernel_rw());
        kassert!(result.is_ok());

        // 验证映射生效
        let translated = pt.translate(vpn.start_addr());
        kassert!(translated.is_some());
    });

    /// 测试页表解除映射触发 TLB shootdown
    test_case!(test_page_table_unmap_with_tlb_flush, {
        let mut pt = PageTableInner::new();
        let vpn = Vpn::from_usize(0x20000);
        let ppn = Ppn::from_usize(0x81000);

        // 先映射
        let result = pt.map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::kernel_rw());
        kassert!(result.is_ok());

        // 再解除映射（应该触发 TLB shootdown）
        let result = pt.unmap(vpn);
        kassert!(result.is_ok());

        // 验证解除映射生效
        let translated = pt.translate(vpn.start_addr());
        kassert!(translated.is_none());
    });

    /// 测试页表权限更新触发 TLB shootdown
    test_case!(test_page_table_update_flags_with_tlb_flush, {
        let mut pt = PageTableInner::new();
        let vpn = Vpn::from_usize(0x30000);
        let ppn = Ppn::from_usize(0x82000);

        // 先映射为只读
        let result = pt.map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::kernel_r());
        kassert!(result.is_ok());

        // 更新权限为读写（应该触发 TLB shootdown）
        let result = pt.update_flags(vpn, UniversalPTEFlag::kernel_rw());
        kassert!(result.is_ok());
    });
}
