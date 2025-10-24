// HACK: 在一个模块目录/文件的顶层又声明了一个同名子模块，这会造成 “module inception”。
// 虽然功能上可行，但会引起 API/模块层次混淆，Clippy 建议消除这种重复。
#![allow(clippy::module_inception)]
mod page_table;
mod page_table_entry;

pub use page_table::*;
pub use page_table_entry::*;

pub type ActivePageTableInner = crate::arch::mm::PageTableInner;

/// Supported page sizes
pub enum PageSize {
    Size4K = 0x1000,
    Size2M = 0x20_0000,
    Size1G = 0x4000_0000,
    // ban bigger sizes for now
}

/// Errors that can occur during paging operations
#[derive(Debug)]
pub enum PagingError {
    /// The virtual address is not mapped
    NotMapped,
    /// The virtual address is already mapped
    AlreadyMapped,
    /// Invalid address provided
    InvalidAddress,
    /// The operation failed due to a conflict with an existing huge page mapping.
    HugePageConflict,
    /// Invalid Flags provided
    InvalidFlags,
    /// Failed to alloc frame
    FrameAllocFailed,
    /// Unsupported mapping type for this operation
    UnsupportedMapType,
    /// Cannot shrink the area below its start
    ShrinkBelowStart,
    /// Huge page splitting is not implemented
    HugePageSplitNotImplemented,
}

pub type PagingResult<T> = Result<T, PagingError>;

