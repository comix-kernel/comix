//! 任务管理器模块
//!
//! 该模块定义了任务管理器的接口和实现
//! 任务管理器负责管理系统中的所有任务
//! 包括任务的创建、销毁和查找等功能
//! 内部维护一个任务映射表，使用任务 ID 作为键
//! 并提供分配唯一任务 ID 的功能
//! 注意：该模块的实例应当被包装在适当的同步原语中以确保线程安全
use alloc::collections::btree_map::BTreeMap;
use alloc::vec::Vec;

use crate::kernel::task::SharedTask;
use crate::kernel::task::tid_allocator::TidAllocator;
use crate::kernel::{TaskState, exit_task_with_block, wake_up_with_block};
use crate::sync::SpinLock;
use crate::uapi::signal::SignalFlags;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref TASK_MANAGER: SpinLock<TaskManager> = SpinLock::new(TaskManager::new());
}

/// 任务管理器接口
///
/// 任务管理器负责所有与任务数据结构相关的修改。
/// 具体来说，它负责以下几项工作：
/// 1. 填写返回值（退出状态）。在 exit 流程中，任务管理器将进程的退出码写入其进程描述符中。
/// 2. 数据结构维护： 维护进程描述符（task_struct）中的所有数据，如 PID、父子关系、权限、打开的文件列表等。
/// 3. 任务生命周期管理： 负责任务的创建、销毁和查找等功能。
/// 注意：任务运行状态的修改由调度器负责
pub trait TaskManagerTrait {
    /// 创建一个新的任务管理器实例
    /// 返回值: TaskManager 结构体
    /// 该实例初始化了任务 ID 分配器和任务映射表
    fn new() -> Self;

    /// 分配一个唯一的任务 ID
    /// 返回值: 分配的任务 ID
    fn allocate_tid(&mut self) -> u32;

    /// 将一个任务添加到任务管理器中
    /// 参数:
    /// * `task`: 需要添加的任务，类型为 SharedTask
    fn add_task(&mut self, task: SharedTask);

    /// 将一个任务标记为退出
    /// 参数:
    /// * `tid`: 需要退出的任务 ID
    fn exit_task(&mut self, task: SharedTask, code: i32);

    /// 释放一个已退出的任务
    /// 参数:
    /// * `task`: 需要释放的任务，类型为 SharedTask
    fn release_task(&mut self, task: SharedTask);

    /// 根据任务 ID 获取对应的任务
    /// 参数:
    /// * `tid`: 需要获取的任务 ID
    ///   返回值: 如果找到对应任务则返回 Some(SharedTask)，否则返回 None
    fn get_task(&self, tid: u32) -> Option<SharedTask>;

    /// 根据条件获取符合条件的任务列表
    /// 参数:
    /// * `cond`: 用于筛选任务的条件函数，接受一个 SharedTask 参数并返回 bool
    /// 返回值: 符合条件的任务列表
    fn get_task_cond(&self, cond: impl Fn(&SharedTask) -> bool) -> Vec<SharedTask>;

    /// 获取进程（线程组）内所有线程
    /// 参数：
    /// * `pid`: 进程 ID
    /// 返回值: 该进程内所有线程的列表
    fn get_process_threads(&self, process: SharedTask) -> Vec<SharedTask>;

    /// 获取进程的所有子进程
    /// 参数：
    /// * `pid`: 进程 ID
    /// 返回值: 该进程的所有子进程列表
    fn get_process_children(&self, process: SharedTask) -> Vec<SharedTask>;

    /// 发送信号给指定任务
    /// 参数：
    /// * `task`: 目标任务对应的 SharedTask
    /// * `signal`: 需要发送的信号编号
    /// 返回值: 如果任务存在且信号发送成功则返回 true，否则返回 false
    fn send_signal(&self, task: SharedTask, signal: usize) -> bool;

    /// 获取所有任务
    /// 返回值: 所有任务的列表
    fn get_all_tasks(&self) -> Vec<SharedTask>;

    /// 获取当前所有进程（线程组 leader）的 PID 快照
    ///
    /// 返回值按升序排列，且去重。
    fn list_process_pids_snapshot(&self) -> Vec<u32>;

    #[cfg(test)]
    /// 获取当前任务数量（仅用于测试）
    /// 返回值: 当前任务数量
    fn task_count(&self) -> usize;
}

/// 任务管理器，负责管理所有任务的生命周期和调度
/// 包括任务的创建、销毁和查找等功能
/// 内部维护一个任务映射表，使用任务 ID 作为键
/// 并提供分配唯一任务 ID 的功能
/// 注意：该结构体的实例应当被包装在适当的同步原语中以确保线程安全
pub struct TaskManager {
    tid_allocator: TidAllocator,
    tasks: BTreeMap<u32, SharedTask>,
}

impl TaskManagerTrait for TaskManager {
    fn new() -> Self {
        TaskManager {
            tid_allocator: TidAllocator::new(),
            tasks: BTreeMap::new(),
        }
    }

    fn allocate_tid(&mut self) -> u32 {
        self.tid_allocator.allocate()
    }

    fn add_task(&mut self, task: SharedTask) {
        let tid = task.lock().tid;
        self.tasks.insert(tid, task);
    }

    fn exit_task(&mut self, task: SharedTask, code: i32) {
        {
            let mut task = task.lock();
            task.exit_code = Some(code as i32);
        }
        exit_task_with_block(task);
    }

    fn release_task(&mut self, task: SharedTask) {
        self.tasks.remove(&task.lock().tid);
    }

    fn get_task(&self, tid: u32) -> Option<SharedTask> {
        self.tasks.get(&tid).cloned()
    }

    fn get_task_cond(&self, cond: impl Fn(&SharedTask) -> bool) -> Vec<SharedTask> {
        let mut v = Vec::new();
        for task in self.tasks.values() {
            if cond(task) {
                v.push(task.clone());
            }
        }
        v
    }

    fn get_process_threads(&self, process: SharedTask) -> Vec<SharedTask> {
        let mut v = Vec::new();
        let pid = process.lock().pid;
        for task in self.tasks.values() {
            if task.lock().pid == pid {
                v.push(task.clone());
            }
        }
        v
    }

    fn get_process_children(&self, process: SharedTask) -> Vec<SharedTask> {
        process.lock().children.lock().clone()
    }

    fn send_signal(&self, task: SharedTask, signal: usize) -> bool {
        if let Some(signal_flag) = SignalFlags::from_signal_num(signal) {
            let mut t = task.lock();
            t.pending.signals.insert(signal_flag);
            if t.state == TaskState::Interruptible {
                drop(t);
                wake_up_with_block(task.clone());
            }
            true
        } else {
            false
        }
    }

    fn get_all_tasks(&self) -> Vec<SharedTask> {
        self.tasks.values().cloned().collect()
    }

    fn list_process_pids_snapshot(&self) -> Vec<u32> {
        // 只列出线程组 leader（进程）：pid == tid
        let mut pids: Vec<u32> = self
            .tasks
            .values()
            .filter_map(|t| {
                let t = t.lock();
                if t.pid == t.tid { Some(t.pid) } else { None }
            })
            .collect();
        pids.sort_unstable();
        pids.dedup();
        pids
    }

    #[cfg(test)]
    fn task_count(&self) -> usize {
        self.tasks.len()
    }
}

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;

    use super::*;
    use crate::{
        kassert,
        kernel::{TaskState, task::TaskStruct},
        sync::SpinLock,
        test_case,
    };

    fn new_dummy_task(tid: u32) -> SharedTask {
        let task = TaskStruct::new_dummy_task(tid);
        Arc::new(SpinLock::new(task))
    }

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

    // 对不存在的 tid 进行查询与退出
    test_case!(test_task_manager_get_remove_nonexistent, {
        let mut tm = TaskManager::new();
        // 查询不存在的任务
        kassert!(tm.get_task(42).is_none());

        // 删除不存在的任务（应为 no-op）
        tm.exit_task(new_dummy_task(42), 0);
        kassert!(tm.get_task(42).is_none());
    });

    // 关于 add_task/get_task/exit_task 的正向测试
    test_case!(test_task_manager_add_get_exit, {
        let mut tm = TaskManager::new();
        let tid = tm.allocate_tid();
        let task = new_dummy_task(tid);
        tm.add_task(task.clone());
        kassert!(tm.get_task(tid).is_some());

        const EXIT_CODE: i32 = 42;

        // 任务管理器执行退出操作（设置返回值和通知调度器）
        tm.exit_task(task, EXIT_CODE);

        let exited_task = tm.get_task(tid).unwrap();
        let g = exited_task.lock();

        // 验证任务管理器设置了返回值 (新的责任)
        kassert!(g.exit_code == Some(EXIT_CODE as i32));

        // 验证调度器设置了状态 (调度器的责任)
        kassert!(g.state == TaskState::Zombie);
    });

    // 释放已退出任务的测试
    test_case!(test_task_manager_release_task, {
        let mut tm = TaskManager::new();
        let tid = tm.allocate_tid();
        let task = new_dummy_task(tid);
        tm.add_task(task.clone());

        // 任务退出（此时状态为 Zombie，仍在 tasks 列表中）
        tm.exit_task(task.clone(), 0);
        kassert!(tm.task_count() == 1);

        // 释放任务
        tm.release_task(task);
        kassert!(tm.task_count() == 0);
        kassert!(tm.get_task(tid).is_none());
    });
}
