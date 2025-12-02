//! 页表项模块
//!
//! 本模块定义了页表项（Page Table Entry, PTE）的通用接口和标志集，
//! 以支持不同体系结构的页表实现。
#![allow(dead_code)]
use crate::mm::address::Ppn;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// 定义了一组通用的页表项标志，可以映射到各种体系结构。
    /// 其低 8 位与 Risc-V SV39 兼容。（如果其他架构需要，可添加更多标志位）
    pub struct UniversalPTEFlag: usize {

            // ---- RISC-V SV39 兼容标志 ----
            const VALID = 1 << 0;               // 指示该页表项是否有效
            const READABLE = 1 << 1;            // 指示该页是否可读
            const WRITEABLE = 1 << 2;           // 指示该页是否可写
            const EXECUTABLE = 1 << 3;          // 指示该页是否可执行
            const USER_ACCESSIBLE = 1 << 4;     // 指示该页是否可从用户模式访问
            const GLOBAL = 1 << 5;              // 指示该页是否为全局页（Global）
            const ACCESSED = 1 << 6;            // 指示该页是否已被访问
            const DIRTY = 1 << 7;               // 指示该页是否已被写入（修改）

            // ---- 额外的通用标志 ----
            #[allow(dead_code)] // TODO(暂时注释): 巨页支持已暂时禁用
            const HUGE = 1 << 8;                // 指示该页是否为巨页（Huge Page）
    }
}

impl UniversalPTEFlag {
    /// 构造用户只读访问的标志集合
    pub const fn user_read() -> Self {
        Self::VALID
            .union(Self::READABLE)
            .union(Self::USER_ACCESSIBLE)
    }

    /// 构造用户读写访问的标志集合
    pub const fn user_rw() -> Self {
        Self::VALID
            .union(Self::READABLE)
            .union(Self::WRITEABLE)
            .union(Self::USER_ACCESSIBLE)
    }

    /// 构造用户读执行访问的标志集合
    pub const fn user_rx() -> Self {
        Self::VALID
            .union(Self::READABLE)
            .union(Self::EXECUTABLE)
            .union(Self::USER_ACCESSIBLE)
    }

    /// 构造内核读写访问的标志集合
    pub const fn kernel_rw() -> Self {
        Self::VALID.union(Self::READABLE).union(Self::WRITEABLE)
    }

    /// 构造内核只读访问的标志集合
    pub const fn kernel_r() -> Self {
        Self::VALID.union(Self::READABLE)
    }

    /// 构造内核读执行访问的标志集合
    pub const fn kernel_rx() -> Self {
        Self::VALID.union(Self::READABLE).union(Self::EXECUTABLE)
    }
}

/// 允许将特定架构的标志与通用标志集相互转换的 trait
pub trait UniversalConvertableFlag {
    /// 从通用标志集创建特定架构的标志
    fn from_universal(flag: UniversalPTEFlag) -> Self;
    /// 将特定架构的标志转换为通用标志集
    fn to_universal(&self) -> UniversalPTEFlag;
}

/// 页表项（Page Table Entry, PTE）所需实现的核心接口
pub trait PageTableEntry {
    /// 用于表示页表项的底层位模式类型
    type Bits;

    /// 从位模式创建页表项
    fn from_bits(bits: Self::Bits) -> Self;
    /// 获取页表项的位模式
    fn to_bits(&self) -> Self::Bits;
    /// 创建一个空的（无效的）页表项
    fn empty() -> Self;
    /// 创建一个新的叶节点（普通页映射）
    fn new_leaf(ppn: Ppn, flags: UniversalPTEFlag) -> Self;
    /// 创建一个新的表节点（指向下一级页表）
    fn new_table(ppn: Ppn) -> Self;

    /// 检查页表项是否有效
    fn is_valid(&self) -> bool;
    /// 检查页表项是否为巨页映射（如果支持）
    fn is_huge(&self) -> bool;
    /// 检查页表项是否为空
    fn is_empty(&self) -> bool;

    /// 获取页表项中存储的物理页号（Ppn）
    fn ppn(&self) -> Ppn;
    /// 获取页表项的通用标志
    fn flags(&self) -> UniversalPTEFlag;

    /// 设置页表项的物理页号
    fn set_ppn(&mut self, ppn: Ppn);
    /// 设置页表项的标志
    fn set_flags(&mut self, flags: UniversalPTEFlag);
    /// 清空页表项（使其无效）
    fn clear(&mut self);

    /// 移除指定的标志
    // current_flags & !flags
    fn remove_flags(&mut self, flags: UniversalPTEFlag);

    /// 添加指定的标志
    // current_flags | flags
    fn add_flags(&mut self, flags: UniversalPTEFlag);
}
