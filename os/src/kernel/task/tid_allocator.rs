//! 简单的任务ID分配器实现

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kassert, test_case};

    // 顺序分配测试：检查分配值从1开始并依次递增
    test_case!(test_tid_allocate_sequence, {
        let alloc = TidAllocator::new();
        kassert!(alloc.allocate() == 1);
        kassert!(alloc.allocate() == 2);
        kassert!(alloc.allocate() == 3);
    });

    // 多引用调用测试：通过多个引用连续分配，确保值唯一且递增
    test_case!(test_tid_allocate_multiple_refs, {
        let alloc = TidAllocator::new();
        let a1 = alloc.allocate();
        let a2 = alloc.allocate();
        kassert!(a1 == 1);
        kassert!(a2 == 2);

        // 通过另一个不可变引用继续分配
        let r = &alloc;
        let a3 = r.allocate();
        kassert!(a3 == 3);
    });
}
