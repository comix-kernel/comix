//! 任务模块
//!
//! 包含任务的创建、调度、终止等功能
//! 并由任务管理器维护所有任务的信息
use core::sync::atomic::Ordering;

mod cap;
mod cred;
mod exec_loader;
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
pub use exec_loader::*;
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

    // 该函数目前仅用于处理“用户态致命异常”，默认行为应与 Linux 类似：
    // 终止整个进程（线程组），并唤醒父进程的 wait。
    let exit_code = code as i32;
    let (pid, is_process) = {
        let t = task.lock();
        (t.pid, t.is_process())
    };
    if is_process {
        cleanup_current_process_resources_on_exit();
        exit_process(task, exit_code);
    } else {
        // 当前为线程：找到线程组 leader（pid==tid）并终止整个进程
        let leader = TASK_MANAGER.lock().get_task(pid);
        if let Some(leader) = leader {
            if leader.lock().is_process() {
                exit_process(leader, exit_code);
            } else {
                TASK_MANAGER.lock().exit_task(task, exit_code);
            }
        } else {
            TASK_MANAGER.lock().exit_task(task, exit_code);
        }
    }
    schedule();
    unreachable!("terminate_task: should not return after scheduled out terminated task");
}

/// 进程退出时的资源清理（Linux 语义子集）：
/// - 释放用户地址空间（页表 + 用户映射）
/// - 关闭打开文件描述符（包括 socket fd）
///
/// 说明：
/// - 仅对“进程/线程组 leader”执行（`task.is_process()`），避免误伤线程共享资源。
/// - 必须先切换到内核页表，再释放当前进程页表资源。
pub fn cleanup_current_process_resources_on_exit() {
    let task = current_task();
    if !task.lock().is_process() {
        return;
    }

    // 1) 先切换到全局内核页表，避免释放“正在使用的 satp”。
    if let Some(kernel_space) = crate::mm::get_global_kernel_space() {
        let _guard = crate::sync::PreemptGuard::new();
        current_cpu().switch_space(kernel_space);
    }

    // 2) 关闭所有 fd，并清理 socket 的 (tid,fd)->handle 映射，避免 fd 复用指向陈旧 handle。
    let tid = { task.lock().tid as usize };
    let fd_table = { task.lock().fd_table.clone() };
    let open = fd_table.take_all();
    for (fd, file) in open {
        if file
            .as_any()
            .downcast_ref::<crate::net::socket::SocketFile>()
            .is_some()
        {
            crate::net::socket::unregister_socket_fd(tid, fd);
        }
        drop(file);
    }

    // 3) 释放用户地址空间。
    task.lock().memory_space = None;
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
