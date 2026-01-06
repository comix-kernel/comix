//! 任务队列模块
//!
//! 定义了任务队列结构体及其相关操作
use alloc::{sync::Arc, vec::Vec};

use crate::kernel::task::SharedTask;

/// 任务队列
/// 用于存放任务
#[derive(Debug)]
pub struct TaskQueue {
    queue: Vec<SharedTask>,
}

impl TaskQueue {
    /// 创建一个新的队列
    pub fn new() -> Self {
        TaskQueue { queue: Vec::new() }
    }

    /// 创建一个空的任务队列（const 版本）
    /// 用于静态数组初始化
    pub const fn empty() -> Self {
        TaskQueue { queue: Vec::new() }
    }

    /// 获取队列中的任务数量
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// 向运行队列添加任务
    pub fn add_task(&mut self, task: SharedTask) {
        self.queue.push(task);
    }

    /// 从运行队列中移除任务
    pub fn remove_task(&mut self, task: &SharedTask) {
        self.queue.retain(|t| !Arc::ptr_eq(t, task));
    }

    /// 从运行队列中弹出一个任务
    pub fn pop_task(&mut self) -> Option<SharedTask> {
        if !self.queue.is_empty() {
            Some(self.queue.remove(0))
        } else {
            None
        }
    }

    /// 检查任务是否在队列中
    pub fn contains(&self, task: &SharedTask) -> bool {
        for t in &self.queue {
            if Arc::ptr_eq(t, task) {
                return true;
            }
        }
        false
    }

    /// 检查队列是否为空
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kassert, kernel::task::TaskStruct, test_case};
    use alloc::sync::Arc;

    fn mk_task(tid: u32) -> SharedTask {
        TaskStruct::new_dummy_task(tid).into_shared()
    }

    // 基础：新增后应存在于队列，队列非空
    test_case!(test_task_queue_add_and_contains, {
        let mut q = TaskQueue::new();
        kassert!(q.is_empty());

        let t1 = mk_task(1);
        let t2 = mk_task(2);

        q.add_task(t1.clone());
        q.add_task(t2.clone());

        kassert!(q.contains(&t1));
        kassert!(q.contains(&t2));
        kassert!(!q.is_empty());
    });

    // FIFO 顺序：先入先出
    test_case!(test_task_queue_pop_order_fifo, {
        let mut q = TaskQueue::new();
        let t1 = mk_task(10);
        let t2 = mk_task(11);
        q.add_task(t1.clone());
        q.add_task(t2.clone());

        let p1 = q.pop_task().expect("expected first task");
        let p2 = q.pop_task().expect("expected second task");
        kassert!(Arc::ptr_eq(&p1, &t1));
        kassert!(Arc::ptr_eq(&p2, &t2));
        kassert!(q.pop_task().is_none());
        kassert!(q.is_empty());
    });

    // 移除：从中间删除应仅移除指定项
    test_case!(test_task_queue_remove_task, {
        let mut q = TaskQueue::new();
        let t1 = mk_task(20);
        let t2 = mk_task(21);
        let t3 = mk_task(22);
        q.add_task(t1.clone());
        q.add_task(t2.clone());
        q.add_task(t3.clone());

        q.remove_task(&t2);
        kassert!(!q.contains(&t2));
        kassert!(q.contains(&t1));
        kassert!(q.contains(&t3));

        // 弹出剩下的两项，顺序应保持相对顺序（t1 -> t3）
        let p1 = q.pop_task().unwrap();
        let p2 = q.pop_task().unwrap();
        kassert!(Arc::ptr_eq(&p1, &t1));
        kassert!(Arc::ptr_eq(&p2, &t3));
        kassert!(q.is_empty());
    });

    // contains 语义：同一 Arc 克隆应视为相等；不同对象即便 tid 相同也不相等
    test_case!(test_task_queue_contains_arc_identity, {
        let mut q = TaskQueue::new();
        let t1 = mk_task(30);
        let t1_clone = t1.clone();
        let t1_other = mk_task(30); // 不同实例（即便 tid 相同）

        q.add_task(t1.clone());
        kassert!(q.contains(&t1));
        kassert!(q.contains(&t1_clone));
        kassert!(!q.contains(&t1_other));
    });

    // 空队列状态切换
    test_case!(test_task_queue_empty_state, {
        let mut q = TaskQueue::new();
        kassert!(q.is_empty());
        let t = mk_task(40);
        q.add_task(t.clone());
        kassert!(!q.is_empty());
        let _ = q.pop_task();
        kassert!(q.is_empty());
    });
}
