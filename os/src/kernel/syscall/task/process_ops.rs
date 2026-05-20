use super::*;

/// 获取当前任务的进程 ID
/// # 返回值:
/// - 进程 ID
pub fn get_pid() -> c_int {
    current_task().lock().pid as c_int
}

/// 获取当前任务的父进程 ID
/// # 返回值:
/// - 父进程 ID, 该进程要么是创建该进程的进程, 要么是重新归属的父进程
pub fn get_ppid() -> c_int {
    current_task().lock().ppid as c_int
}

/// 获取进程组 ID
///
/// # 参数
/// - `pid`: 进程 ID。如果为 0，返回调用进程的 PGID
///
/// # 返回值
/// - 成功: 返回进程组 ID
/// - 失败: 返回 -ESRCH (进程不存在或 pid 为负数)
pub fn get_pgid(pid: c_int) -> c_int {
    use crate::uapi::errno::ESRCH;

    if pid == 0 {
        return current_task().lock().pgid as c_int;
    }

    if pid < 0 {
        return -ESRCH as c_int;
    }

    let manager = TASK_MANAGER.lock();
    let task_opt = manager.get_task(pid as u32);
    drop(manager);

    match task_opt {
        Some(task) => task.lock().pgid as c_int,
        None => -ESRCH as c_int,
    }
}

/// 设置进程组 ID
pub fn set_pgid(pid: c_int, pgid: c_int) -> c_int {
    use crate::uapi::errno::{EINVAL, EPERM, ESRCH};

    let current = current_task();
    let current_locked = current.lock();
    let current_pid = current_locked.tid as c_int;
    let _current_ppid = current_locked.ppid as c_int;
    drop(current_locked);

    let target_pid = if pid == 0 { current_pid } else { pid };
    let target_pgid = if pgid == 0 { target_pid } else { pgid };

    if target_pgid < 0 {
        return -EINVAL as c_int;
    }

    let manager = TASK_MANAGER.lock();
    let task_opt = manager.get_task(target_pid as u32);
    drop(manager);

    let task = match task_opt {
        Some(t) => t,
        None => return -ESRCH as c_int,
    };

    let mut task_locked = task.lock();

    // 权限检查：目标进程必须是调用者本身或其子进程
    if target_pid != current_pid && task_locked.ppid as c_int != current_pid {
        return -ESRCH as c_int;
    }

    // 不能更改已经是会话领导者的进程（简化检查：pgid == tid）
    if task_locked.pgid == task_locked.tid {
        return -EPERM as c_int;
    }

    // 设置进程组 ID
    task_locked.pgid = target_pgid as u32;
    0
}

/// 获取资源限制
/// # 参数
/// - `resource`: 资源限制 ID
/// - `rlim`: 指向 rlimit 结构体的指针, 用于存储获取到的资源限制
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn getrlimit(resource: c_int, rlim: *mut Rlimit) -> c_int {
    if resource as usize >= RLIM_NLIMITS {
        return -EINVAL;
    }
    let rlimit = current_task().lock().rlimit.lock().limits[resource as usize];
    unsafe {
        write_to_user(rlim, rlimit);
    }
    0
    // TODO: EPERM 和 EFAULT
}

/// 设置资源限制
/// # 参数
/// - `resource`: 资源限制 ID
/// - `rlim`: 指向 rlimit 结构体的指针, 包含要设置的资源限制
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn setrlimit(resource: c_int, rlim: *const Rlimit) -> c_int {
    if resource as usize >= RLIM_NLIMITS {
        return -EINVAL;
    }
    let new_limit = unsafe { read_from_user(rlim) };
    if new_limit.rlim_cur > new_limit.rlim_max {
        return -EINVAL;
    }
    {
        let rlimit_lock = current_task().lock().rlimit.clone();
        rlimit_lock.lock().limits[resource as usize] = new_limit;
    }
    0
    // TODO: EPERM, EPERM 和 EFAULT
}

/// 获取或设置资源限制
/// # 参数
/// - `pid`: 目标进程 ID, 为 0 表示当前进程
/// - `resource`: 资源限制 ID
/// - `new_limit`: 指向 rlimit 结构体的指针, 包含要设置的资源限制, 若不设置则为 NULL
/// - `old_limit`: 指向 rlimit 结构体的指针, 用于存储获取到的资源限制, 若不获取则为 NULL
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn prlimit(
    pid: c_int,
    resource: c_int,
    new_limit: *const Rlimit,
    old_limit: *mut Rlimit,
) -> c_int {
    if resource as usize >= RLIM_NLIMITS {
        return -EINVAL;
    }
    let target_task = if pid == 0 {
        current_task()
    } else {
        let tm = TASK_MANAGER.lock();
        match tm.get_task(pid as u32) {
            Some(t) => t,
            None => return -ESRCH,
        }
    };

    if !old_limit.is_null() {
        let rlimit = target_task.lock().rlimit.lock().limits[resource as usize];
        unsafe {
            write_to_user(old_limit, rlimit);
        }
    }

    if !new_limit.is_null() {
        let new_rlim = unsafe { read_from_user(new_limit) };
        if new_rlim.rlim_cur > new_rlim.rlim_max {
            return -EINVAL;
        }
        let rlimit_lock = target_task.lock().rlimit.clone();
        rlimit_lock.lock().limits[resource as usize] = new_rlim;
    }

    0
    // TODO: EPERM, EPERM 和 EFAULT
}
