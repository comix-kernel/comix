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
