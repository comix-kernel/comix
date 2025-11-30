//! 进程模块
//!
//! 已经采用多任务设计的内核，进程作为任务的一种特殊形式存在。
//! 故此模块变得相对简单，主要负责适配传统的进程概念与内核任务之间的关系。

use crate::{
    kernel::{SharedTask, TASK_MANAGER, TaskManagerTrait, notify_parent},
    uapi::signal::SignalFlags,
};

/// 进程退出处理
/// 该函数负责清理进程资源并通知父进程，
/// 如果该进程有子进程，处理孤儿进程
/// 如果该进程有线程，处理线程退出
/// # 参数：
/// * `task` - 进程对应的任务
/// * `code` - 退出码
pub fn exit_process(task: SharedTask, code: i32) {
    if !task.lock().is_process() {
        panic!("exit_process called on a non-process task");
    }
    let (children, threads, init_task) = {
        let mut t = TASK_MANAGER.lock();
        t.exit_task(task.clone(), code);
        (
            t.get_process_children(task.clone()),
            t.get_process_threads(task.clone()),
            t.get_task(1).expect("init process not found"),
        )
    };
    {
        let init = init_task.lock();
        let mut pchild = init.children.lock();
        for child in children {
            let mut c = child.lock();
            pchild.push(child.clone());
            c.ppid = init.pid;
        }
    }
    {
        let mut t = TASK_MANAGER.lock();
        for thread in threads {
            t.exit_task(thread, 0x114514);
        }
    }
    notify_parent(task);
}

/// 向进程发送信号
/// # 参数：
/// * `task` - 目标进程对应的任务
/// * `sig` - 要发送的信号编号
pub fn send_signal_process(task: &SharedTask, sig: usize) {
    task.lock()
        .shared_pending
        .lock()
        .signals
        .insert(SignalFlags::from_signal_num(sig).unwrap());
}
