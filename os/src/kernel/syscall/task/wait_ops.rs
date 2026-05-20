use super::*;

/// 等待子进程状态变化（wait4）
/// # 说明
/// 状态变化包括：
///     1. 子进程已终止；
///     2. 子进程被信号停止；
///     3. 子进程被信号恢复。
/// 对于已终止的子进程，执行等待操作可以让系统释放与该子进程关联的资源；
/// # 参数
/// - `pid`:
///     1. > 0 表示需要等待的子进程ID
///     2. 0 表示等待同一进程组的任意子进程
///     3. -1 表示等待任意子进程
///     4. < -1 表示等待进程组ID等于 pid 绝对值的任意子进程
/// - `wstatus`: 指向存储子进程状态的整数指针
/// - `options`: 等待选项标志
/// - `rusage`: 指向存储资源使用情况的 rusage 结构体指针
/// # 返回值
/// - 成功返回子进程 ID, 如果设置了 NOHANG 标志且没有满足条件的子进程，则立即返回 0，失败返回负错误码
/// TODO:
/// 1. rusage 参数的处理
/// 2. 信号处理
/// 3. 错误处理
pub fn wait4(pid: c_int, wstatus: *mut c_int, options: c_int, _rusage: *mut Rusage) -> c_int {
    // 阻塞当前任务,直到指定的子任务结束
    let cur_task = current_task();
    let opt = if let Some(opt) = WaitFlags::from_bits(options as usize) {
        opt
    } else {
        return -EINVAL;
    };
    let cur_pgid = cur_task.lock().pgid;
    let match_pid = |child_task: &SharedTask| {
        match pid {
            -1 => true,                                           // 匹配所有子进程
            0 => child_task.lock().pgid == cur_pgid,              // 匹配进程组
            p if p > 0 => child_task.lock().pid == p as u32,      // 匹配特定 PID
            p if p < -1 => child_task.lock().pgid == (-p) as u32, // 匹配特定进程组 |pid|
            _ => unreachable!("wait4: unreachable pid match case."),
        }
    };
    let check_exited = opt.contains(WaitFlags::EXITED)
        || (!opt.contains(WaitFlags::STOPPED) && !opt.contains(WaitFlags::CONTINUED));

    let zombie: fn(TaskState) -> bool = if check_exited {
        |ch| ch == TaskState::Zombie
    } else {
        |_ch| false
    };
    let continued: fn(TaskState) -> bool = if opt.contains(WaitFlags::CONTINUED) {
        |ch| ch == TaskState::Running
    } else {
        |_ch| false
    };
    let stopped: fn(TaskState) -> bool = if opt.contains(WaitFlags::STOPPED) {
        |ch| ch == TaskState::Stopped || ch == TaskState::Zombie
    } else {
        |_ch| false
    };
    let cond = |ch: &SharedTask| {
        if !match_pid(ch) {
            return false;
        }
        let state = ch.lock().state;
        zombie(state) || continued(state) || stopped(state)
    };

    let task = loop {
        let mut found: Option<SharedTask> = None;
        let mut nohang = false;

        let slept = sleep_task_prepare(cur_task.clone(), true, |t| {
            if let Some(res) = t.check_child(cond, !opt.contains(WaitFlags::NOWAIT)) {
                crate::pr_debug!("wait4: found child pid={}", res.lock().pid);
                found = Some(res);
                return true;
            }
            if opt.contains(WaitFlags::NOHANG) {
                nohang = true;
                return true;
            }
            let mut wc = t.wait_child.lock();
            if !wc.contains(&cur_task) {
                wc.add_task(cur_task.clone());
            }
            false
        });

        if !slept {
            if let Some(res) = found {
                break res;
            }
            if nohang {
                return 0;
            }
        }
        yield_task();
    };

    let (tid, state, exit_code) = {
        let t = task.lock();
        (t.tid, t.state, t.exit_code)
    };

    let status = match state {
        TaskState::Zombie => {
            // TODO: 处理信号退出的情况
            WaitStatus::exit_code(exit_code.expect("Zombie must set exit code.") as u8, 0)
        }
        TaskState::Stopped => {
            WaitStatus::stop_code(0) // TODO: 停止信号
        }
        TaskState::Running => WaitStatus::continued_code(),
        _ => {
            unreachable!("wait4: unexpected task state.")
        }
    };

    // wstatus 允许为 NULL（例如 waitpid(-1, NULL, 0)），此时不写回状态
    if !wstatus.is_null() {
        unsafe {
            write_to_user(wstatus, status.raw());
        }
    }

    // 如果子任务是 Zombie 状态，从 TASK_MANAGER 中释放它
    // 这样 Task 结构体和其拥有的资源（kstack, trap_frame）才会被释放
    // 当 wait4 使用 WNOWAIT 标志调用时，它不应该回收子进程
    if state == TaskState::Zombie && !opt.contains(WaitFlags::NOWAIT) {
        // [FIX] 从父进程的 children 列表中移除该任务
        // 之前只从 wait_child 移除了，导致 children 列表一直持有引用，造成泄漏
        // XX: 这是否是必要的修复?
        {
            let parent = current_task();
            let p_lock = parent.lock();
            let mut children = p_lock.children.lock();
            let old_len = children.len();
            children.retain(|c| c.lock().tid != tid);
            crate::pr_debug!(
                "[wait4] Removed from parent.children: {} -> {}",
                old_len,
                children.len()
            );
        }
        TASK_MANAGER.lock().release_task(task);
    }

    tid as c_int
}
