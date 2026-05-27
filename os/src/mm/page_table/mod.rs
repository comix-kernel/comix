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
}

/// 分页操作中可能发生的错误
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingError {
    /// 虚拟地址未被映射
    NotMapped,
    /// 虚拟地址已被映射
    AlreadyMapped,
    /// 提供了无效的地址
    InvalidAddress,
    /// 提供了无效的标志（Flags）
    InvalidFlags,
    /// 页表权限不允许该访问
    PermissionDenied,
    /// 帧（Frame）分配失败
    FrameAllocFailed,
    /// 此操作不支持此映射类型
    #[allow(dead_code)]
    UnsupportedMapType,
    /// 区域不能收缩到其起始地址以下
    #[allow(dead_code)]
    ShrinkBelowStart,
    /// 内存耗尽
    OutOfMemory,
}

/// 分页操作的结果类型
pub type PagingResult<T> = Result<T, PagingError>;
