//! 页表模块
//!
//! 本模块提供与页表管理相关的功能，包括页表的创建、映射、解除映射、翻译等操作。
mod inner;
mod page_table_entry;

pub use inner::*;
pub use page_table_entry::*;

// 活动页表内部类型别名
pub type ActivePageTableInner = crate::arch::mm::PageTableInner;

/// 支持的页大小
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSize {
    Size4K = 0x1000,
    #[allow(dead_code)] // TODO(暂时注释): 大页支持已暂时禁用
    Size2M = 0x20_0000,
    #[allow(dead_code)] // TODO(暂时注释): 大页支持已暂时禁用
    Size1G = 0x4000_0000,
    // 暂时禁止更大的页大小
}

/// 分页操作中可能发生的错误
#[derive(Debug)]
pub enum PagingError {
    /// 虚拟地址未被映射
    NotMapped,
    /// 虚拟地址已被映射
    AlreadyMapped,
    /// 提供了无效的地址
    InvalidAddress,
    /// 由于与现有的巨页（Huge Page）映射冲突，操作失败。
    #[allow(dead_code)] // TODO(暂时注释): 大页支持已暂时禁用
    HugePageConflict,
    /// 提供了无效的标志（Flags）
    InvalidFlags,
    /// 帧（Frame）分配失败
    FrameAllocFailed,
    /// 此操作不支持此映射类型
    #[allow(dead_code)]
    UnsupportedMapType,
    /// 区域不能收缩到其起始地址以下
    #[allow(dead_code)]
    ShrinkBelowStart,
    /// 巨页拆分功能尚未实现
    #[allow(dead_code)] // TODO(暂时注释): 大页支持已暂时禁用
    HugePageSplitNotImplemented,
    /// 内存耗尽
    OutOfMemory,
}

/// 分页操作的结果类型
pub type PagingResult<T> = Result<T, PagingError>;
