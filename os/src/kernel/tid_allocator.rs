#![allow(dead_code)]
use core::sync::atomic::AtomicU32;

/// 简单的任务ID分配器。
/// 每次调用`allocate`方法时，都会返回一个唯一的任务ID。
/// 任务ID从1开始递增。
#[derive(Debug)]
pub struct TidAllocator {
    next_tid: AtomicU32,
}

impl TidAllocator {
    /// 创建一个新的TidAllocator实例。
    pub const fn new() -> Self {
        TidAllocator {
            next_tid: AtomicU32::new(1),
        }
    }

    /// 分配一个新的任务ID。
    pub fn allocate(&self) -> u32 {
        self.next_tid
            .fetch_add(1, core::sync::atomic::Ordering::SeqCst)
    }
}
