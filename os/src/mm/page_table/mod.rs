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
}

pub type PagingResult<T> = Result<T, PagingError>;

