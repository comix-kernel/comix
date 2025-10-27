use crate::mm::frame_allocator::{FrameTracker, frame_allocator::FRAME_ALLOCATOR};

// -------------------------------------------------------------------
// 用于导出到外部的 C 风格的物理页分配接口
// -------------------------------------------------------------------

/// 分配一个物理页面，返回对应的 FrameTracker。
/// # Returns
/// 成功则返回 Some(FrameTracker)，失败则返回 None。
/// # Example
/// ```ignore
/// let frame = physical_page_alloc();
/// ```
pub fn physical_page_alloc() -> Option<FrameTracker> {
    FRAME_ALLOCATOR.lock().alloc_frame()
}
