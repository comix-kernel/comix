use core::sync::atomic::Ordering;

use alloc::sync::Arc;
use lazy_static::lazy_static;

mod ktask;
mod task_manager;
mod task_state;
mod task_struct;
mod tid_allocator;

pub use ktask::*;
pub use task_state::TaskState;
pub use task_struct::Task as TaskStruct;

pub type SharedTask = Arc<SpinLock<TaskStruct>>;

use crate::{
    arch::trap::{TrapFrame, restore},
    kernel::{cpu::current_cpu, schedule, task::task_manager::TaskManager},
    sync::SpinLock,
};

lazy_static! {
    static ref TASK_MANAGER: SpinLock<TaskManager> = SpinLock::new(TaskManager::new());
}

/// 把已初始化的 TaskStruct 包装为共享任务句柄
pub fn into_shared(task: TaskStruct) -> SharedTask {
    Arc::new(SpinLock::new(task))
}

/// 新创建的线程发生第一次调度时会从 forkret 开始执行
/// 该函数负责恢复任务的陷阱帧，从而进入任务的实际执行上下文
pub(crate) fn forkret() {
    let fp: *mut TrapFrame;
    {
        let cpu = current_cpu().lock();
        let task = cpu.current_task.as_ref().unwrap();
        fp = task.lock().trap_frame_ptr.load(Ordering::SeqCst);
    }
    unsafe { restore(&*fp) };
}

/// 在任务结束时调用的函数
/// 任务正常地执行完毕后通过创建时预先设置的寄存器跳转到该函数
/// 该函数不会返回，负责清理任务资源并切换到下一个任务
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
        t.state = TaskState::Stopped;
        t.exit_code = t_exit_code;
        t.return_value = t_return_value;
        // println!("terminate_task: task {} terminated, exit_code={:?}, return_value={:?}", t.tid, t.exit_code, t.return_value);
    }
    drop(task);
    schedule();
    unreachable!("terminate_task: should not return after scheduled out terminated task");
}
