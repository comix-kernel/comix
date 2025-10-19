mod frame_allocator;

pub use frame_allocator::{FrameRangeTracker, FrameTracker};

pub fn init_frame_allocator(start_addr: usize, end_addr: usize) {
    frame_allocator::init_frame_allocator(start_addr, end_addr);
}