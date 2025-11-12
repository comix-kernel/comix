//! 任务模块
//!
//! 包含任务的创建、调度、终止等功能
//! 并由任务管理器维护所有任务的信息
use core::sync::atomic::Ordering;

mod ktask;
mod task_manager;
mod task_state;
mod task_struct;
mod tid_allocator;

pub use ktask::*;
pub use task_manager::{TASK_MANAGER, TaskManagerTrait};
pub use task_state::TaskState;
pub use task_struct::SharedTask;
pub use task_struct::Task as TaskStruct;

use alloc::sync::Arc;

use crate::{
    arch::trap::{TrapFrame, restore},
    kernel::{cpu::current_cpu, schedule},
    sync::SpinLock,
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
    // SAFETY: fp 指向的内存已经被分配且由当前任务拥有
    unsafe { restore(&*fp) };
}

/// 在任务结束时调用的函数
/// 任务正常地执行完毕后通过创建时预先设置的寄存器跳转到该函数
/// 该函数不会返回，负责清理任务资源并切换到下一个任务
/// 参数:
/// * `return_value`: 任务的返回值
pub(crate) fn terminate_task(return_value: usize) -> ! {
    let task = {
        let cpu = current_cpu().lock();
        let task = cpu.current_task.as_ref().unwrap().clone();
        task
    };

    {
        let mut t = task.lock();
        // 设置退出码和返回值
        // 对于进程，设置 exit_code；对于线程，设置 return_value
        let (t_exit_code, t_return_value) = if t.is_process() {
            (Some(return_value as i32), None)
        } else {
            (None, Some(return_value))
        };
        // 不必将task移出cpu,在schedule时会处理
        t.state = TaskState::Zombie;
        t.exit_code = t_exit_code;
        t.return_value = t_return_value;
    }
    drop(task);
    schedule();
    unreachable!("terminate_task: should not return after scheduled out terminated task");
}

/// 获取当前task
///
/// 获取后须手动lock
pub fn current_task() -> SharedTask {
    current_cpu()
        .lock()
        .current_task
        .as_ref()
        .expect("current_task: CPU has no current task")
        .clone()
}

/// 获取当前任务的文件描述符表
pub fn current_fd_table() -> Arc<FDTable> {
    current_task().lock().fd_table.clone()
}

/// 从当前任务的 FD 表中获取文件
pub fn get_file(fd: usize) -> Result<Arc<File>, FsError> {
    current_fd_table().get(fd)
}
