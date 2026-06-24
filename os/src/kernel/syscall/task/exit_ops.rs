use super::*;

/// 线程退出系统调用
/// # 说明
/// 终止调用该系统调用的执行流（即线程）
/// 对于非主线程, 该线程立即终止。内核回收该线程的栈和其他线程特定的资源。
/// 进程中的其他线程继续正常执行。
/// 对于主线程, 该线程终止整个进程，
/// 除非进程中有其他线程调用 execve() 或等待其子线程终止(TODO: 该行为待验证)
/// # 参数
/// - `code`: 退出代码
pub fn exit(code: c_int) -> c_int {
    // TODO: 处理 tid_addr 和 robust_list
    clear_child_tid_and_wake();
    let task = current_task();
    // Linux 语义：退出时释放用户地址空间并关闭打开文件。
    // 注意：必须先切换到内核页表，再释放当前进程页表资源，避免释放“正在使用的 satp”。
    crate::kernel::task::cleanup_current_process_resources_on_exit();
    if task.lock().is_process() {
        exit_process(task, code & 0xFF);
    } else {
        TASK_MANAGER.lock().exit_task(task, code & 0xFF);
    }
    schedule();
    unreachable!("exit: exit_task should not return.");
}

/// 进程 (线程组) 退出系统调用
/// # 说明
/// exit_group() 函数将"立即"终止调用进程。该进程拥有的所有打开文件描述符均被关闭。
/// 该进程的所有子进程将由 init(1) 进程（TODO: 或通过 prctl(2) 的
/// PR_SET_CHILD_SUBREAPER 操作定义的最近"子进程回收器"进程）继承。
/// 进程父进程将收到 SIGCHLD 信号。
/// 返回值 code & 0xFF作为进程退出状态传递给父进程，
/// 父进程可通过wait(2)系列调用之一获取该状态。
/// # 参数
/// - `code`: 退出代码
pub fn exit_group(code: c_int) -> ! {
    // TODO: 处理 tid_addr 和 robust_list
    clear_child_tid_and_wake();
    let task = current_task();
    let leader = crate::kernel::task::task_group_leader(&task).unwrap_or(task);
    crate::kernel::task::cleanup_process_resources_on_exit(leader.clone());
    exit_process(leader, code & 0xFF);
    schedule();
    unreachable!("exit: exit_task should not return.");
}

fn clear_child_tid_and_wake() {
    let task = current_task();
    let clear_addr = {
        let mut t = task.lock();
        // 避免重复清理
        t.clear_child_tid.take()
    };

    let Some(clear_addr) = clear_addr else { return };

    // If the task has already dropped its user address space (e.g. forced exit paths),
    // we cannot touch userspace or translate the address. Best-effort: just skip.
    let memory_space = {
        let t = task.lock();
        t.memory_space.clone()
    };
    let Some(memory_space) = memory_space else {
        return;
    };

    // 1) write 0 to userspace tid address
    unsafe {
        write_to_user(clear_addr.as_usize() as *mut c_int, 0);
    }

    // 2) futex wake
    let Some(paddr) = memory_space
        .lock()
        .translate(VA::from_usize(clear_addr.as_usize()))
        .map(|p| p.as_usize())
    else {
        return;
    };
    FUTEX_MANAGER.lock().get_wait_queue(paddr).wake_up_all();
}
