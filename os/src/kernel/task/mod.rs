//! 任务模块
//!
//! 包含任务的创建、调度、终止等功能
//! 并由任务管理器维护所有任务的信息
use core::sync::atomic::Ordering;

mod cap;
mod cred;
mod futex;
mod ktask;
mod process;
mod task_manager;
mod task_state;
mod task_struct;
mod tid_allocator;
mod work_queue;

pub use cap::*;
pub use cred::*;
pub use futex::*;
pub use ktask::*;
pub use process::*;
pub use task_manager::{TASK_MANAGER, TaskManagerTrait};
pub use task_state::TaskState;
pub use task_struct::FsStruct;
pub use task_struct::SharedTask;
pub use task_struct::Task as TaskStruct;
pub use work_queue::*;

use alloc::sync::Arc;

use crate::mm::memory_space::MemorySpace;
use crate::sync::SpinLock;
use crate::uapi::signal::NUM_SIGCHLD;
use crate::{
    arch::trap::{TrapFrame, restore},
    kernel::{cpu::current_cpu, schedule},
    vfs::{FDTable, File, FsError},
};

/// 新创建的线程发生第一次调度时会从 forkret 开始执行
/// 该函数负责恢复任务的陷阱帧，从而进入任务的实际执行上下文
pub(crate) fn forkret() {
    let fp: *mut TrapFrame;
    {
        let cpu = current_cpu().lock();
        let task = cpu.current_task.as_ref().unwrap();
        fp = task.lock().trap_frame_ptr.load(Ordering::SeqCst);
    }
    #[cfg(target_arch = "loongarch64")]
    {
        crate::arch::trap::set_trap_frame_ptr(fp as usize);
    }
    // SAFETY: fp 指向的内存已经被分配且由当前任务拥有
    unsafe { restore(&*fp) };
}

/// 在任务结束时调用的函数
/// 任务正常地执行完毕后通过创建时预先设置的寄存器跳转到该函数
/// 该函数不会返回，负责清理任务资源并切换到下一个任务
/// 参数:
/// * `code`: 任务的退出码
pub(crate) fn terminate_task(code: usize) -> ! {
    let task = {
        let cpu = current_cpu().lock();
        cpu.current_task.as_ref().unwrap().clone()
    };

    {
        let mut t = task.lock();
        // 不必将task移出cpu,在schedule时会处理
        t.state = TaskState::Zombie;
        t.exit_code = Some(code as i32);
    }
    drop(task);
    schedule();
    unreachable!("terminate_task: should not return after scheduled out terminated task");
}

/// 获取当前task
/// # 返回值：当前任务的SharedTask
pub fn current_task() -> SharedTask {
    current_cpu()
        .lock()
        .current_task
        .as_ref()
        .expect("current_task: CPU has no current task")
        .clone()
}

/// 获取当前任务的内存空间
/// # 返回值：当前任务的内存空间
pub fn current_memory_space() -> Arc<SpinLock<MemorySpace>> {
    current_cpu()
        .lock()
        .current_memory_space
        .as_ref()
        .expect("current_memory_space: current task has no memory space")
        .clone()
}

/// 通知父任务子任务状态变化
/// # 参数：
/// * `task`: 子任务
pub fn notify_parent(task: SharedTask) {
    let ppid = {
        let t = task.lock();
        t.ppid
    };

    let t = TASK_MANAGER.lock();
    if let Some(p) = t.get_task(ppid) {
        // 1. 发送信号 (Wake up signal path)
        // 注意：send_signal 会短暂获取 p 锁
        t.send_signal(p.clone(), NUM_SIGCHLD);

        // 2. 唤醒等待队列 (WaitQueue path)
        // 必须显式唤醒，因为 sys_wait4 等待在 wait_child 上
        // 这里的关键是不能持有 p 锁调用 wake_up，否则死锁 (Recursive Parent Lock)
        let wait_child = p.lock().wait_child.clone();
        // 释放 p 锁 (t lock 也在 get_task 后如果 drop? 不，t held here)
        // t is TASK_MANAGER lock. p.lock() is Parent lock.
        // We hold TM lock. We hold Parent lock (briefly).
        // Then we hold WaitQueue lock.
        // wait_child.lock().wake_up_one() -> Locks Scheduler -> Locks Parent.
        // If we hold TM lock: TM -> WaitQueue -> Parent.
        // Is Parent -> TM possible? No.

        // So this is safe.
        wait_child.lock().wake_up_one();
    } else {
        // Parent not found (e.g. init process exiting), ignore
        let pid = task.lock().pid;
        crate::pr_warn!(
            "notify_parent: parent task {} not found for child {}",
            ppid,
            pid
        );
    }
}

/// 获取当前任务的文件描述符表
pub fn current_fd_table() -> Arc<FDTable> {
    current_task().lock().fd_table.clone()
}

/// 从当前任务的 FD 表中获取文件
pub fn get_file(fd: usize) -> Result<Arc<dyn File>, FsError> {
    current_fd_table().get(fd)
}
