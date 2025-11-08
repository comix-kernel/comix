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
    ///   返回值: 如果找到对应任务则返回 Some(SharedTask)，否则返回 None
    pub fn get_task(&self, tid: u32) -> Option<SharedTask> {
        self.tasks.get(&tid).cloned()
    }

    #[cfg(test)]
    /// 获取当前任务数量（仅用于测试）
    /// 返回值: 当前任务数量
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kassert, kernel::task::TaskStruct, sync::spin_lock::SpinLock, test_case};

    // 通过 TaskManager 分配 tid：应从 1 开始递增
    test_case!(test_task_manager_allocate_sequence, {
        let mut tm = TaskManager::new();
        let t1 = tm.allocate_tid();
        let t2 = tm.allocate_tid();
        let t3 = tm.allocate_tid();
        kassert!(t1 == 1);
        kassert!(t2 == 2);
        kassert!(t3 == 3);
    });

    // 对不存在的 tid 进行查询与删除：不应崩溃，查询为 None
    test_case!(test_task_manager_get_remove_nonexistent, {
        let mut tm = TaskManager::new();
        // 查询不存在的任务
        kassert!(tm.get_task(42).is_none());

        // 删除不存在的任务（应为 no-op）
        tm.remove_task(42);
        kassert!(tm.get_task(42).is_none());
    });

    // 关于 add_task/get_task 的正向测试
    test_case!(test_task_manager_add_get_remove, {
        let mut tm = TaskManager::new();
        let tid = tm.allocate_tid();
        let task = new_dummy_task(tid);
        tm.add_task(task.clone());
        kassert!(tm.get_task(tid).is_some());
        tm.remove_task(tid);
        kassert!(tm.get_task(tid).is_none());
    });

    fn new_dummy_task(tid: u32) -> SharedTask {
        use alloc::sync::Arc;
        let task = TaskStruct::new_dummy_task(tid);
        Arc::new(SpinLock::new(task))
    }
}
