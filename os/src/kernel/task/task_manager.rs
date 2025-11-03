use alloc::collections::btree_map::BTreeMap;

use crate::kernel::task::SharedTask;
use crate::kernel::task::tid_allocator::TidAllocator;

/// 任务管理器，负责管理所有任务的生命周期和调度
/// 包括任务的创建、销毁和查找等功能
/// 内部维护一个任务映射表，使用任务 ID 作为键
/// 并提供分配唯一任务 ID 的功能
/// 注意：该结构体的实例应当被包装在适当的同步原语中以确保线程安全
pub struct TaskManager {
    tid_allocator: TidAllocator,
    tasks: BTreeMap<u32, SharedTask>,
}

impl TaskManager {
    /// 创建一个新的任务管理器实例
    /// 返回值: TaskManager 结构体
    /// 该实例初始化了任务 ID 分配器和任务映射表
    pub fn new() -> Self {
        TaskManager {
            tid_allocator: TidAllocator::new(),
            tasks: BTreeMap::new(),
        }
    }

    /// 分配一个唯一的任务 ID
    /// 返回值: 分配的任务 ID
    pub fn allocate_tid(&mut self) -> u32 {
        self.tid_allocator.allocate()
    }

    /// 将一个任务添加到任务管理器中
    /// 参数:
    /// * `task`: 需要添加的任务，类型为 SharedTask
    pub fn add_task(&mut self, task: SharedTask) {
        let tid = task.lock().tid;
        self.tasks.insert(tid, task);
    }

    /// 从任务管理器中移除一个任务
    /// 参数:
    /// * `tid`: 需要移除的任务 ID
    pub fn remove_task(&mut self, tid: u32) {
        self.tasks.remove(&tid);
    }

    /// 根据任务 ID 获取对应的任务
    /// 参数:
    /// * `tid`: 需要获取的任务 ID
    /// 返回值: 如果找到对应任务则返回 Some(SharedTask)，否则返回 None
    pub fn get_task(&self, tid: u32) -> Option<SharedTask> {
        self.tasks.get(&tid).cloned()
    }
}
