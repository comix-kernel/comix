use crate::mm::frame_allocator::FrameTracker;

/// 分配一个物理页面，返回对应的 FrameTracker。
/// # Returns
/// 成功则返回 Some(FrameTracker)，失败则返回 None。
/// # Example
/// ```ignore
/// let frame = physical_page_alloc();
/// ```
pub fn physical_page_alloc() -> Option<FrameTracker> {
    unimplemented!("physical_page_alloc 尚未实现")
}
