//! Futex 相关功能

use hashbrown::HashMap;

use crate::{kernel::WaitQueue, sync::SpinLock};

lazy_static::lazy_static! {
    /// 全局 Futex 管理器实例
    pub static ref FUTEX_MANAGER: SpinLock<FutexManager> = SpinLock::new(FutexManager::new());
}

/// Futex 管理器，负责管理所有的 Futex 对象
pub struct FutexManager {
    futexes: HashMap<usize, WaitQueue>,
}

impl FutexManager {
    /// 创建一个新的 Futex 管理器实例
    pub fn new() -> Self {
        Self {
            futexes: HashMap::new(),
        }
    }

    /// 根据用户空间地址获取对应的 Futex 等待队列
    pub fn get_wait_queue(&mut self, uaddr: usize) -> &mut WaitQueue {
        self.futexes.entry(uaddr).or_insert_with(WaitQueue::new)
    }
}
