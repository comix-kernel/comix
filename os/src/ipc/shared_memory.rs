//! 共享内存模块
//!
//! 提供共享物理页的分配与映射到当前进程用户空间的能力。

use alloc::{sync::Arc, vec::Vec};

use crate::{
    config::PAGE_SIZE,
    kernel::{current_cpu, current_task},
    mm::{
        frame_allocator::{FrameTracker, alloc_frames},
        page_table::{PagingError, UniversalPTEFlag},
    },
};

/// 共享内存表：简单管理若干共享段
pub struct SharedMemoryTable {
    memory: Vec<Arc<SharedMemory>>,
}

impl SharedMemoryTable {
    /// 创建共享内存表
    pub fn new() -> Self {
        Self { memory: Vec::new() }
    }

    /// 新建共享段并登记，返回 Arc 句柄
    pub fn create(&mut self, pages: usize) -> Arc<SharedMemory> {
        let shm = Arc::new(SharedMemory::new(pages));
        self.memory.push(shm.clone());
        shm
    }

    /// 简单移除（若还被其他地方持有 Arc，不会真正释放）
    pub fn remove(&mut self, shm: &Arc<SharedMemory>) -> bool {
        if let Some(i) = self.memory.iter().position(|x| Arc::ptr_eq(x, shm)) {
            self.memory.swap_remove(i);
            // XXX: 是不是还应取消在当前进程用户空间上的映射
            true
        } else {
            false
        }
    }

    /// 当前已登记的共享段数量
    pub fn len(&self) -> usize {
        self.memory.len()
    }

    pub fn is_empty(&self) -> bool {
        self.memory.is_empty()
    }
}

/// 共享内存：持有一组物理页（FrameTracker）
pub struct SharedMemory {
    frames: Vec<FrameTracker>,
    len: usize,
}

impl SharedMemory {
    /// 分配 pages 个物理页作为共享段
    pub fn new(pages: usize) -> Self {
        let frames = alloc_frames(pages).expect("unable to alloc shared memory");
        SharedMemory {
            frames,
            len: pages * PAGE_SIZE,
        }
    }

    /// 共享段字节数
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 将共享段映射到当前进程用户空间
    /// 返回
    /// - Ok(usize) 成功；Err(PagingError) 失败
    pub fn map_to_user(self) -> Result<usize, PagingError> {
        let current = current_task();
        let mut task = current.lock();
        let space = task
            .memory_space
            .as_mut()
            .expect("map_to_user_at: task has no user memory space");

        let flags = UniversalPTEFlag::READABLE
            | UniversalPTEFlag::WRITEABLE
            | UniversalPTEFlag::USER_ACCESSIBLE
            | UniversalPTEFlag::VALID;

        space.lock().mmap(0, self.len, flags)
    }
}
