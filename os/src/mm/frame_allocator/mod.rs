// HACK: 在一个模块目录/文件的顶层又声明了一个同名子模块，这会造成 “module inception”。
// 虽然功能上可行，但会引起 API/模块层次混淆，Clippy 建议消除这种重复。
#![allow(clippy::module_inception)]
//! Frame allocator module
//!
//! This module provides physical memory frame allocation and tracking functionality.
//!
//! # Components
//!
//! - [`FrameTracker`]: RAII wrapper for single allocated frames
//! - [`FrameRangeTracker`]: RAII wrapper for ranges of allocated frames
//! - [`init_frame_allocator`]: Initialize the global frame allocator
//! - [`alloc_frame`]: Allocate a single frame
//! - [`alloc_frames`]: Allocate multiple (non-contiguous) frames
//! - [`alloc_contig_frames`]: Allocate multiple contiguous frames

mod frame_allocator;

/// initialize the global frame allocator with the available physical memory range
///
/// # Parameters
///
/// * `start_addr` - start address of the available physical memory
/// * `end_addr` - end address of the available physical memory
pub fn init_frame_allocator(start_addr: usize, end_addr: usize) {
    frame_allocator::init_frame_allocator(start_addr, end_addr);
}

pub use frame_allocator::FrameTracker;