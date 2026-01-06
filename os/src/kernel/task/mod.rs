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
    arch::trap::restore,
    kernel::{cpu::current_cpu, schedule},
    vfs::{FDTable, File, FsError},
};

#[cfg(not(target_arch = "loongarch64"))]
use crate::arch::trap::TrapFrame;

/// 新创建的线程发生第一次调度时会从 forkret 开始执行
/// 该函数负责恢复任务的陷阱帧，从而进入任务的实际执行上下文
#[cfg(target_arch = "loongarch64")]
pub(crate) fn forkret() {
    let (tf_ptr, is_kernel_thread) = {
        let _guard = crate::sync::PreemptGuard::new();
        let cpu = current_cpu();
        let task = cpu
            .current_task
            .as_ref()
            .expect("forkret: CPU has no current task")
            .clone();
        let t = task.lock();
        (
            t.trap_frame_ptr.load(Ordering::SeqCst),
            t.memory_space.is_none(),
        )
    };

    // LoongArch: 内核线程首次运行不走 “ertn” 的异常返回路径，
    // 否则会错误改写 CRMD/翻译模式导致卡死；直接切换栈并跳转到 entry 即可。
    if is_kernel_thread {
        // SAFETY: tf_ptr 指向当前任务拥有的 TrapFrame；entry/sp 由创建逻辑填充
        let (entry, sp) = unsafe { ((*tf_ptr).era, (*tf_ptr).kernel_sp) };
        unsafe {
            core::arch::asm!(
                "addi.d $sp, {sp}, 0",
                "jirl $zero, {entry}, 0",
                sp = in(reg) sp,
                entry = in(reg) entry,
                options(noreturn)
            );
        }
    }

    // 用户任务：仍然通过 restore->ertn 进入用户态
    // SAFETY: tf_ptr 指向的内存已经被分配且由当前任务拥有
    unsafe { restore(&*tf_ptr) };
}

#[cfg(not(target_arch = "loongarch64"))]
pub(crate) fn forkret() {
    let fp: *mut TrapFrame;
    {
        let _guard = crate::sync::PreemptGuard::new();
        let cpu = current_cpu();
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
/// * `code`: 任务的退出码
pub(crate) fn terminate_task(code: usize) -> ! {
    let task = {
        let _guard = crate::sync::PreemptGuard::new();
        let cpu = current_cpu();
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

/// 尝试获取当前task
/// # 返回值：当前任务的SharedTask，如果没有则返回None
pub fn try_current_task() -> Option<SharedTask> {
    let _guard = crate::sync::PreemptGuard::new();
    current_cpu().current_task.as_ref().cloned()
}

/// 获取当前task
/// # 返回值：当前任务的SharedTask
/// # Panics：如果当前CPU没有任务则panic
pub fn current_task() -> SharedTask {
    match try_current_task() {
        Some(task) => task,
        None => {
            // 打印调用栈信息以便调试
            crate::pr_err!("current_task called with no current task!");
            crate::pr_err!("CPU ID: {}", crate::arch::kernel::cpu::cpu_id());
            panic!("current_task: CPU has no current task")
        }
    }
}

/// 获取当前任务的内存空间
/// # 返回值：当前任务的内存空间
pub fn current_memory_space() -> Arc<SpinLock<MemorySpace>> {
    let _guard = crate::sync::PreemptGuard::new();
    current_cpu()
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
