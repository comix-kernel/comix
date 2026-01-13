//! 任务相关的系统调用实现

use core::{
    ffi::{c_char, c_int, c_ulong, c_void},
    sync::atomic::Ordering,
};

use alloc::{string::ToString, sync::Arc, vec::Vec};

use crate::{
    arch::{
        timer::{clock_freq, get_time},
        trap::{SumGuard, restore},
    },
    ipc::{SignalHandlerTable, SignalPending, signal_pending},
    kernel::{
        FUTEX_MANAGER, Scheduler, SharedTask, TASK_MANAGER, TIMER, TIMER_QUEUE, TaskManagerTrait,
        TaskState, TaskStruct, TimerEntry, current_cpu, current_task, exit_process, schedule,
        sleep_task_with_block, sleep_task_with_guard_and_block,
        syscall::util::{get_args_safe, get_path_safe},
        time::{REALTIME, realtime_now},
        yield_task,
    },
    mm::{
        address::{UsizeConvert, Vaddr},
        frame_allocator::{alloc_contig_frames, alloc_frame},
        memory_space::MemorySpace,
    },
    sync::SpinLock,
    uapi::{
        errno::{
            EAGAIN, EFAULT, EINTR, EINVAL, EIO, EISDIR, ENOENT, ENOEXEC, ENOMEM, ENOSYS, EPERM,
            ESRCH, ETIMEDOUT,
        },
        futex::{FUTEX_CLOCK_REALTIME, FUTEX_PRIVATE, FUTEX_WAIT, FUTEX_WAKE, RobustListHead},
        resource::{RLIM_NLIMITS, Rlimit, Rusage},
        sched::CloneFlags,
        signal::{NUM_SIGALRM, NUM_SIGPROF, NUM_SIGVTALRM},
        time::{
            Itimerval, TimeSpec,
            clock_flags::TIMER_ABSTIME,
            clock_id::{
                CLOCK_BOOTTIME, CLOCK_MONOTONIC, CLOCK_PROCESS_CPUTIME_ID, CLOCK_REALTIME,
                CLOCK_TAI,
            },
            itimer_id::{ITIMER_PROF, ITIMER_REAL, ITIMER_VIRTUAL},
        },
        types::{SizeT, StackT},
        wait::{WaitFlags, WaitStatus},
    },
    util::user_buffer::{read_from_user, write_to_user},
    vfs::FsError,
};

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
    crate::kernel::task::cleanup_current_process_resources_on_exit();
    exit_process(current_task(), code & 0xFF);
    schedule();
    unreachable!("exit: exit_task should not return.");
}

fn clear_child_tid_and_wake() {
    let task = current_task();
    let clear_addr = {
        let mut t = task.lock();
        let addr = t.clear_child_tid;
        // 避免重复清理
        t.clear_child_tid = 0;
        addr
    };

    if clear_addr == 0 {
        return;
    }

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
        let _guard = SumGuard::new();
        write_to_user(clear_addr as *mut c_int, 0);
    }

    // 2) futex wake
    let Some(paddr) = memory_space
        .lock()
        .translate(Vaddr::from_usize(clear_addr as usize))
        .map(|p| p.as_usize())
    else {
        return;
    };
    FUTEX_MANAGER.lock().get_wait_queue(paddr).wake_up_all();
}

/// 克隆当前任务（线程或进程）
/// # 参数
/// - `fn_ptr`: 指向新任务执行函数的指针
/// - `stack`: 指向新任务栈顶的指针
/// - `flags`: 克隆选项标志
/// - `arg`: 传递给新任务执行函数的参数指针
/// # 返回值
/// - 成功返回新任务的线程 ID (TID)，失败返回负错误码
pub fn clone(
    flags: c_ulong,   // a0: clone flags
    stack: c_ulong,   // a1: child stack pointer
    ptid: *mut c_int, // a2: parent_tid pointer
    ctid: *mut c_int, // a3: child_tid pointer
    tls: *mut c_void, // a4: TLS pointer
) -> c_int {
    let requested_flags = if let Some(requested_flags) = CloneFlags::from_bits(flags as usize) {
        requested_flags
    } else {
        return -EINVAL;
    };
    if !requested_flags.is_known() {
        return -EINVAL;
    }
    if !requested_flags.is_supported() {
        return -ENOSYS;
    }
    // 根据 clone(2) 的 man page，当指定 CLONE_VM 标志时，必须为子进程提供一个新的栈
    // 否则父子进程将共享同一个栈，导致栈污染和程序崩溃
    if requested_flags.contains(CloneFlags::VM) && stack == 0 {
        return -EINVAL;
    }
    let tid = { TASK_MANAGER.lock().allocate_tid() };
    let (
        c_pid,
        c_ppid,
        c_pgid,
        space,
        signal_handlers,
        blocked,
        signal,
        signal_stack,
        ptf,
        fd_table,
        fs,
        uts,
        rlimit,
    ) = {
        let _guard = crate::sync::PreemptGuard::new();
        let cpu = current_cpu();
        let task = cpu.current_task.as_ref().unwrap().lock();
        (
            task.pid,
            task.ppid,
            task.pgid,
            task.memory_space
                .clone()
                .expect("fork: can only call fork on a user task."),
            task.signal_handlers.clone(),
            task.blocked,
            task.shared_pending.clone(),
            task.signal_stack.clone(),
            task.trap_frame_ptr.load(Ordering::SeqCst),
            task.fd_table.clone(),
            task.fs.clone(),
            task.uts_namespace.clone(),
            task.rlimit.clone(),
        )
    };
    let exit_signal = requested_flags.get_exit_signal();
    let space = if requested_flags.contains(CloneFlags::VM) {
        space
    } else {
        let cloned = match space.lock().clone_for_fork() {
            Ok(s) => s,
            Err(_) => {
                // 语义：fork/clone(不带 CLONE_VM) 内存不足时返回 ENOMEM，而不是 panic。
                return -ENOMEM;
            }
        };
        Arc::new(SpinLock::new(cloned))
    };
    let fd_table = if requested_flags.contains(CloneFlags::FILES) {
        fd_table
    } else {
        Arc::new(fd_table.clone_table())
    };
    let fs = if requested_flags.contains(CloneFlags::FS) {
        fs
    } else {
        Arc::new(SpinLock::new(fs.lock().clone()))
    };
    let ppid = if requested_flags.contains(CloneFlags::PARENT) {
        c_ppid
    } else {
        c_pid
    };
    let pid = if requested_flags.contains(CloneFlags::THREAD) {
        c_pid
    } else {
        tid
    };
    let (signal, signal_handler, signal_stack) = if requested_flags.contains(CloneFlags::SIGHAND) {
        (signal, signal_handlers, signal_stack)
    } else {
        (
            Arc::new(SpinLock::new(SignalPending::empty())),
            Arc::new(SpinLock::new(SignalHandlerTable::new())),
            Arc::new(SpinLock::new(StackT::default())),
        )
    };

    let kstack_tracker = alloc_contig_frames(4).expect("fork: alloc kstack failed.");
    let trap_frame_tracker = alloc_frame().expect("fork: alloc trap frame failed");
    let mut child_task = TaskStruct::utask_create(
        tid,
        pid,
        ppid,
        c_pgid,
        TaskStruct::empty_children(),
        kstack_tracker,
        trap_frame_tracker,
        space,
        signal_handler,
        blocked,
        signal,
        signal_stack,
        exit_signal,
        uts,
        rlimit,
        fd_table,
        fs,
    );

    if requested_flags.contains(CloneFlags::CHILD_SETTID) {
        unsafe {
            write_to_user(ctid, tid as c_int);
        }
    }
    if requested_flags.contains(CloneFlags::PARENT_SETTID) {
        unsafe {
            write_to_user(ptid, tid as c_int);
        }
    }

    let tf = child_task.trap_frame_ptr.load(Ordering::SeqCst);
    unsafe {
        (*tf).set_clone_trap_frame(&*ptf, child_task.kstack_base, stack as usize);
        if requested_flags.contains(CloneFlags::SETTLS) {
            #[cfg(target_arch = "riscv64")]
            {
                // RISC-V userspace uses tp register as thread pointer (TLS base)
                (*tf).x4_tp = tls as usize;
            }
            #[cfg(target_arch = "loongarch64")]
            {
                // LoongArch userspace uses r2 (tp) register as thread pointer (TLS base)
                (*tf).regs[2] = tls as usize;
            }
        }
    }
    if requested_flags.contains(CloneFlags::CHILD_SETTID) {
        child_task.set_child_tid = ctid as usize;
    }
    if requested_flags.contains(CloneFlags::CHILD_CLEARTID) {
        child_task.clear_child_tid = ctid as usize;
    }
    let child_task = child_task.into_shared();
    current_task()
        .lock()
        .children
        .lock()
        .push(child_task.clone());

    // 选择目标 CPU（负载均衡）
    let target_cpu = crate::kernel::pick_cpu();
    child_task.lock().on_cpu = Some(target_cpu);

    let child_tid = child_task.lock().tid;
    crate::pr_debug!(
        "[SMP] Task {} (child) assigned to CPU {}",
        child_tid,
        target_cpu
    );

    TASK_MANAGER.lock().add_task(child_task.clone());
    crate::pr_debug!(
        "[SMP] Adding task {} to CPU {} scheduler",
        child_tid,
        target_cpu
    );
    crate::kernel::scheduler_of(target_cpu)
        .lock()
        .add_task(child_task);

    // 如果目标 CPU 不是当前 CPU，发送 IPI
    let current_cpu = crate::arch::kernel::cpu::cpu_id();
    if target_cpu != current_cpu {
        crate::pr_debug!(
            "[SMP] Sending IPI from CPU {} to CPU {}",
            current_cpu,
            target_cpu
        );
        crate::arch::ipi::send_reschedule_ipi(target_cpu);
    }

    tid as c_int
}

/// 执行一个新程序（execve）
/// # 参数
/// - `path`: 可执行文件路径
/// - `argv`: 命令行参数
/// - `envp`: 环境变量
/// TODO: 目前该函数可用但亟待完善
pub fn execve(
    path: *const c_char,
    argv: *const *const c_char,
    envp: *const *const c_char,
) -> c_int {
    // 使用 SumGuard 来安全访问用户空间路径和参数
    let (path_str, argv_strings, envp_strings) = unsafe {
        let _guard = SumGuard::new();
        let path_str = match get_path_safe(path) {
            Ok(s) => s.to_string(),
            Err(_) => {
                return FsError::InvalidArgument.to_errno() as i32;
            }
        };
        let argv_strings = get_args_safe(argv, "argv").unwrap_or_else(|_| Vec::new());
        let envp_strings = get_args_safe(envp, "envp").unwrap_or_else(|_| Vec::new());
        (path_str, argv_strings, envp_strings)
    };

    let mut exec_path_str = path_str.clone();
    let (argv_strings, envp_strings, exec_path_str) = {
        // 只读取文件头部用于 hashbang 判断，避免一次性把整个 ELF 读入内存。
        let dentry = match crate::vfs::vfs_lookup(&path_str) {
            Ok(d) => d,
            Err(FsError::NotFound) => return -ENOENT,
            Err(FsError::IsDirectory) => return -EISDIR,
            Err(_) => return -EIO,
        };
        let inode = dentry.inode.clone();
        let meta = match inode.metadata() {
            Ok(m) => m,
            Err(FsError::NotFound) => return -ENOENT,
            Err(FsError::IsDirectory) => return -EISDIR,
            Err(_) => return -EIO,
        };
        if meta.inode_type != crate::vfs::InodeType::File {
            return -EISDIR;
        }

        let prefix_len = core::cmp::min(meta.size, 256);
        if prefix_len == 0 {
            return -ENOEXEC;
        }
        let mut prefix = alloc::vec![0u8; prefix_len];
        let mut read_total = 0usize;
        while read_total < prefix.len() {
            let n = match inode.read_at(read_total, &mut prefix[read_total..]) {
                Ok(n) => n,
                Err(FsError::NotFound) => return -ENOENT,
                Err(FsError::IsDirectory) => return -EISDIR,
                Err(_) => return -EIO,
            };
            if n == 0 {
                break;
            }
            read_total += n;
        }
        if read_total == 0 {
            return -ENOEXEC;
        }
        prefix.truncate(read_total);

        if prefix.len() >= 2 && prefix[0] == b'#' && prefix[1] == b'!' {
            if let Ok((path, args)) = parse_hashbang(&prefix) {
                let mut new_argv = Vec::new();
                new_argv.push(path.to_string());
                // XXX: 目前仅支持单个参数
                if let Some(arg) = args {
                    new_argv.push(arg.to_string());
                }
                new_argv.push(path_str.clone());
                new_argv.extend(argv_strings.iter().skip(1).cloned());
                exec_path_str = path.to_string();
                (new_argv, envp_strings, exec_path_str)
            } else {
                return -EINVAL;
            }
        } else {
            (argv_strings, envp_strings, exec_path_str)
        }
    };

    // // 构造 &str 切片（String 的所有权在本函数内，切片在调用 t.execve 时仍然有效）
    // let argv_refs: Vec<&str> = argv_strings.iter().map(|s| s.as_str()).collect();
    // let envp_refs: Vec<&str> = envp_strings.iter().map(|s| s.as_str()).collect();

    // /proc/[pid]/exe 使用尽量稳定的绝对路径
    let exe_path = match crate::vfs::vfs_lookup(&exec_path_str) {
        Ok(d) => d.full_path(),
        Err(_) => exec_path_str.clone(),
    };

    // 解析 ELF 并准备新的地址空间（但不切换）
    let (space, initial_pc, sp, phdr_addr, phnum, phent, at_base, at_entry) =
        match do_execve_prepare(&exec_path_str) {
            Ok(res) => res,
            Err(e) => return e,
        };

    drop(path_str);

    // 切换到新的地址空间并恢复到用户态（此函数不会返回）
    do_execve_switch(
        space,
        initial_pc,
        sp,
        exe_path,
        argv_strings, // Pass ownership
        envp_strings, // Pass ownership
        phdr_addr,
        phnum,
        phent,
        at_base,
        at_entry,
    )
}

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
        {
            let mut t = cur_task.lock();
            if let Some(res) = t.check_child(cond, !opt.contains(WaitFlags::NOWAIT)) {
                crate::pr_debug!("wait4: found child pid={}", res.lock().pid);
                break res;
            } else {
                if opt.contains(WaitFlags::NOHANG) {
                    return 0;
                }
            }
            {
                let mut wc = t.wait_child.lock();
                if !wc.contains(&cur_task) {
                    wc.add_task(cur_task.clone());
                }
            }
            sleep_task_with_guard_and_block(&mut t, cur_task.clone(), true);
        }
        // 在没有持有任何锁的情况下调用调度相关操作
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
    use crate::uapi::errno::{EACCES, EINVAL, EPERM, ESRCH};

    let current = current_task();
    let current_locked = current.lock();
    let current_pid = current_locked.tid as c_int;
    let current_ppid = current_locked.ppid as c_int;
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

/// 高精度睡眠（纳秒级别）
/// # 参数
/// - `duration`: 指向 TimeSpec 结构体的指针, 包含睡眠的时间
/// - `rem`: 指向 TimeSpec 结构体的指针, 用于存储剩余的睡眠时间, 可为 NULL
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn nanosleep(duration: *const TimeSpec, rem: *mut TimeSpec) -> c_int {
    let req = unsafe { read_from_user(duration) };
    if req.tv_sec == 0 && req.tv_nsec == 0 {
        return 0;
    }
    if req.tv_sec < 0 || req.tv_nsec < 0 || req.tv_nsec > 999999999 {
        return -EINVAL;
    }
    let mut result = 0;
    let task = current_task();
    let trigger = get_time() + req.into_freq(clock_freq());

    let mut timer_q = TIMER_QUEUE.lock();
    timer_q.push(trigger, task.clone());
    sleep_task_with_block(task.clone(), true);
    drop(timer_q);
    yield_task();

    // 被唤醒后（可能是超时，也可能是信号），必须确保将任务从定时器队列中清理掉
    // 如果是超时唤醒，pop_due_task 已经移除了
    // 如果是信号唤醒，任务还在队列中，需要手动移除以避免 Arc 泄漏
    TIMER_QUEUE.lock().remove_task(&task);

    if !rem.is_null() {
        let dur = trigger.saturating_sub(get_time());
        let remaining_ticks = if dur > 0 {
            // XXX: 提前唤醒是否一定是因为信号？
            result = -EINTR;
            dur
        } else {
            0
        };
        let rem_ts = TimeSpec::from_freq(remaining_ticks, clock_freq());
        unsafe {
            write_to_user(rem, rem_ts);
        }
    }

    result
    // TODO: EFAULT
}

pub fn gettid() -> c_int {
    current_task().lock().tid as c_int
}

/// 基于时钟的高精度睡眠
/// # 参数
/// - `clk_id`: 时钟 ID
/// - `flags`: 睡眠选项标志
/// - `req`: 指向 TimeSpec 结构体的指针, 包含睡眠的时间
/// - `rem`: 指向 TimeSpec 结构体的指针, 用于存储剩余的睡眠时间, 可为 NULL
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn clock_nanosleep(
    clk_id: c_int,
    flags: c_int,
    req: *const TimeSpec,
    rem: *mut TimeSpec,
) -> c_int {
    let time_req = unsafe { read_from_user(req) };
    let is_abstime = (flags & TIMER_ABSTIME) != 0;
    let sleep_ticks = time_req.into_freq(clock_freq());
    let trigger = if is_abstime {
        sleep_ticks
    } else {
        let now = match clk_id {
            CLOCK_REALTIME => REALTIME.read().into_freq(clock_freq()),
            CLOCK_MONOTONIC => get_time(),
            CLOCK_TAI | CLOCK_BOOTTIME | CLOCK_PROCESS_CPUTIME_ID => return -ENOSYS,
            _ => return -EINVAL,
        };
        now.saturating_add(sleep_ticks)
    };

    let mut result = 0;
    let task = current_task();

    let mut timer_q = TIMER_QUEUE.lock();
    timer_q.push(trigger, task.clone());
    sleep_task_with_block(task.clone(), true);
    drop(timer_q);
    yield_task();

    // 同 sys_nanosleep，防止信号唤醒导致的 Arc 泄漏
    TIMER_QUEUE.lock().remove_task(&task);

    if !rem.is_null() {
        let dur = trigger.saturating_sub(get_time());
        let remaining_ticks = if dur > 0 {
            // XXX: 提前唤醒是否一定是因为信号？
            result = -EINTR;
            dur
        } else {
            0
        };
        let rem_ts = TimeSpec::from_freq(remaining_ticks, clock_freq());
        unsafe {
            write_to_user(rem, rem_ts);
        }
    }

    result
}

/// 获取间隔定时器的当前值
/// # 参数
/// - `which`: 定时器 ID
/// - `curr_value`: 指向 Itimerval 结构体的指针, 用于存储当前定时器值
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn getitimer(which: c_int, curr_value: *mut Itimerval) -> c_int {
    match which {
        ITIMER_REAL | ITIMER_VIRTUAL | ITIMER_PROF => {}
        _ => return -EINVAL,
    }
    let sig = match which {
        ITIMER_REAL => NUM_SIGALRM,
        ITIMER_VIRTUAL => NUM_SIGVTALRM,
        ITIMER_PROF => NUM_SIGPROF,
        _ => unreachable!("getitimer: unreachable which case."),
    };
    // Linux semantics: ITIMER_* are per-process (thread group), not per-thread.
    // Use the thread-group leader (pid) as the timer owner.
    let owner = {
        let pid = current_task().lock().pid;
        TASK_MANAGER
            .lock()
            .get_task(pid)
            .unwrap_or_else(current_task)
    };
    let mut val = Itimerval::zero();
    if let Some(timer) = TIMER.lock().find_entry(&owner, sig) {
        let now = get_time();
        let remaining = if *timer.0 > now { *timer.0 - now } else { 0 };
        let it_value = TimeSpec::from_freq(remaining, clock_freq()).to_timeval();
        let it_interval = timer.1.it_interval.to_timeval();
        val = Itimerval {
            it_value,
            it_interval,
        };
    }
    unsafe {
        write_to_user(curr_value, val);
    }
    0
}

/// 设置间隔定时器的值
/// # 参数
/// - `which`: 定时器 ID
/// - `new_value`: 指向 Itimerval 结构体的指针, 包含要设置的定时器值
/// - `old_value`: 指向 Itimerval 结构体的指针, 用于存储旧的定时器值, 可为 NULL
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn setitimer(which: c_int, new_value: *const Itimerval, old_value: *mut Itimerval) -> c_int {
    match which {
        ITIMER_REAL | ITIMER_VIRTUAL | ITIMER_PROF => {}
        _ => return -EINVAL,
    }
    let sig = match which {
        ITIMER_REAL => NUM_SIGALRM,
        ITIMER_VIRTUAL => NUM_SIGVTALRM,
        ITIMER_PROF => NUM_SIGPROF,
        _ => unreachable!("setitimer: unreachable which case."),
    };

    // Linux semantics: ITIMER_* are per-process (thread group), not per-thread.
    let owner = {
        let pid = current_task().lock().pid;
        TASK_MANAGER
            .lock()
            .get_task(pid)
            .unwrap_or_else(current_task)
    };

    let mut binding = TIMER.lock();

    // Linux semantics: return the previous timer value, then replace it with the new one.
    let mut old = Itimerval::zero();
    if let Some(timer) = binding.find_entry(&owner, sig) {
        let now = get_time();
        let remaining = if *timer.0 > now { *timer.0 - now } else { 0 };
        let it_value = TimeSpec::from_freq(remaining, clock_freq()).to_timeval();
        let it_interval = timer.1.it_interval.to_timeval();
        old = Itimerval {
            it_value,
            it_interval,
        };
    }

    // Disarm any existing timers for this (task, sig) so we don't accumulate duplicates.
    while binding.remove_entry(&owner, sig).is_some() {}

    let new_itimer = unsafe { read_from_user(new_value) };
    if !new_itimer.it_value.is_zero() {
        let trigger = get_time() + new_itimer.it_value.into_freq(clock_freq());
        let interval = new_itimer.it_interval.to_timespec();
        let entry = TimerEntry {
            task: owner,
            sig,
            it_interval: interval,
        };
        binding.push(trigger, entry);
    }
    if !old_value.is_null() {
        unsafe {
            write_to_user(old_value, old);
        }
    }

    0
}

/// Futex 系统调用实现
/// # 参数
/// - `uaddr`: 指向用户空间中 futex 变量的指针
/// - `op`: 操作码和标志
/// - `val`: 操作相关的值
/// - `_timeout`: 指向 TimeSpec 结构体的指针, 用于指定超时时间
/// - `_uaddr2`: 指向用户空间中第二个 futex 变量的指针
/// - `_val3`: 额外的操作相关值
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn futex(
    uaddr: *mut u32,
    op: c_int,
    val: u32,
    timeout: *const TimeSpec,
    _uaddr2: *mut u32,
    _val3: u32,
) -> c_int {
    let _private = (op & FUTEX_PRIVATE as c_int) != 0; // TODO: 目前不区分 PRIVATE 和 SHARED
    let realtime = (op & FUTEX_CLOCK_REALTIME as c_int) != 0;
    let op = op & !(FUTEX_PRIVATE as c_int) & !(FUTEX_CLOCK_REALTIME as c_int);
    // HACK: 其实只需要锁定与 uaddr 对应的 Futex 等待队列
    let mut fm = FUTEX_MANAGER.lock();
    match op as u32 {
        FUTEX_WAIT => {
            // 必须保证获 取锁 → 读取用户数据 → 比较 → 释放锁 整个序列是原子的
            let user_val = unsafe { read_from_user(uaddr) };
            let memory_space = current_task()
                .lock()
                .memory_space
                .as_ref()
                .expect("futex: current task has no memory space.")
                .clone();
            let paddr = if let Some(paddr) = memory_space
                .lock()
                .translate(Vaddr::from_usize(uaddr as usize))
            {
                paddr.as_usize()
            } else {
                return -EFAULT;
            };
            if user_val != val as u32 {
                return -EAGAIN;
            }

            let task = current_task();
            let waitq = fm.get_wait_queue(paddr);
            waitq.sleep(task.clone());
            sleep_task_with_block(task.clone(), true);

            if !timeout.is_null() {
                let ts = unsafe { read_from_user(timeout) };
                if ts.tv_sec < 0 || ts.tv_nsec < 0 || ts.tv_nsec > 999999999 {
                    return -EINVAL;
                }
                let sleep_ticks = ts.into_freq(clock_freq());
                let trigger = if realtime {
                    let now = realtime_now().into_freq(clock_freq());
                    now.saturating_add(sleep_ticks)
                } else {
                    let now = get_time();
                    now.saturating_add(sleep_ticks)
                };
                TIMER_QUEUE.lock().push(trigger, task.clone());
                drop(fm);
                yield_task();
                if TIMER_QUEUE.lock().remove_task(&task).is_none() {
                    // 超时唤醒
                    let mut fm = FUTEX_MANAGER.lock();
                    let waitq = fm.get_wait_queue(paddr);
                    // 虽然任务已经被唤醒, 但仍然需要从等待队列中移除
                    waitq.remove_task(&task);
                    return -ETIMEDOUT;
                }
            } else {
                drop(fm);
                yield_task();
            }
            if signal_pending(&task) {
                // 信号唤醒
                let mut fm = FUTEX_MANAGER.lock();
                let waitq = fm.get_wait_queue(paddr);
                waitq.remove_task(&task);
                return -EINTR;
            }
            // 正常唤醒
            // NOTE: 此时任务已经不在等待队列中
            0
        }
        FUTEX_WAKE => {
            let mut wake_count = 0;
            let paddr = {
                let memory_space = current_task()
                    .lock()
                    .memory_space
                    .as_ref()
                    .expect("futex: current task has no memory space.")
                    .clone();
                if let Some(paddr) = memory_space
                    .lock()
                    .translate(Vaddr::from_usize(uaddr as usize))
                {
                    paddr.as_usize()
                } else {
                    return -EFAULT;
                }
            };
            let mut fm = FUTEX_MANAGER.lock();
            let waitq = fm.get_wait_queue(paddr);
            for _ in 0..val {
                waitq.wake_up_one();
                wake_count += 1;
            }
            wake_count
        }
        _ => -ENOSYS,
    }
}

//    long syscall(SYS_get_robust_list, int pid,
//                 struct robust_list_head **head_ptr, size_t *sizep);
//    long syscall(SYS_set_robust_list,
//                 struct robust_list_head *head, size_t size);

/// 设置线程 ID 地址
/// # 参数
/// - `tidptr`: 指向存储线程 ID 的用户空间地址
/// # 返回值
/// - 返回当前线程的线程 ID (TID)
pub fn set_tid_address(tidptr: *mut c_int) -> c_int {
    let task = current_task();
    task.lock().clear_child_tid = tidptr as usize;
    current_task().lock().tid as c_int
}

/// 获取线程的 robust futex 列表头指针和大小
/// # 参数
/// - `pid`: 目标进程 ID, 为 0 表示当前进程
/// - `head_ptr`: 指向存储 robust futex 列表头指针的用户空间地址
/// - `sizep`: 指向存储 robust futex 列表大小的用户空间地址
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn get_robust_list(pid: c_int, head_ptr: *mut *mut RobustListHead, sizep: *mut SizeT) -> c_int {
    let task = if pid == 0 {
        current_task()
    } else {
        let tm = TASK_MANAGER.lock();
        match tm.get_task(pid as u32) {
            Some(t) => t,
            None => return -ESRCH,
        }
    };
    let (head, size) = {
        let t = task.lock();
        let head = match t.robust_list {
            Some(h) => h as *mut RobustListHead,
            None => core::ptr::null_mut(),
        };
        let size = size_of::<RobustListHead>() as SizeT;
        (head, size)
    };
    unsafe {
        write_to_user(head_ptr, head);
        write_to_user(sizep, size);
    }
    0
}

/// 设置线程的 robust futex 列表头指针
/// # 参数
/// - `head`: 指向 robust futex 列表头的指针
/// - `size`: robust futex 列表的大小
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn set_robust_list(head: *const RobustListHead, size: SizeT) -> c_int {
    if size != size_of::<RobustListHead>() as SizeT {
        return -EINVAL;
    }
    let task = current_task();
    task.lock().robust_list = Some(head as usize);
    0
}

/// 创建一个新的会话并设置进程组 ID
/// # 返回值
/// - 成功返回新会话的进程组 ID, 失败返回负错误码
pub fn setsid() -> c_int {
    let task = current_task();
    let mut t = task.lock();
    if t.pid == t.pgid {
        return -EPERM;
    }
    let new_pgid = t.pid;
    t.pgid = new_pgid;
    new_pgid as c_int
}

/// 辅助函数：解析 Hashbang 行
fn parse_hashbang(data: &[u8]) -> Result<(&str, Option<&str>), ()> {
    // 查找第一个换行符 ('\n')，只读取第一行
    let line_end = data.iter().position(|&b| b == b'\n').unwrap_or(data.len());
    // 跳过开头的空格和制表符
    let line_start = data[2..line_end]
        .iter()
        .position(|&b| b != b' ' && b != b'\t')
        .unwrap_or(line_end - 2)
        + 2;
    let line = &data[line_start..line_end];

    // 假设用空格分隔解释器路径和可选参数
    let parts: Vec<&[u8]> = line
        .split(|&b| b == b' ' || b == b'\t')
        .filter(|p| !p.is_empty()) // 过滤空串
        .collect();

    if parts.is_empty() {
        return Err(()); // 格式错误或只包含 #!
    }

    // 解释器路径
    let interpreter_path = core::str::from_utf8(parts[0]).map_err(|_| ())?;

    // 可选参数
    let interpreter_arg = parts
        .get(1)
        .map(|p| core::str::from_utf8(p))
        .transpose()
        .map_err(|_| ())?;

    Ok((interpreter_path, interpreter_arg))
}

/// 执行一个新程序（execve）的准备阶段：解析 ELF 并创建新的地址空间
fn do_execve_prepare(
    path: &str,
) -> Result<
    (
        Arc<SpinLock<MemorySpace>>,
        usize,
        usize,
        usize,
        usize,
        usize,
        usize,
        usize,
    ),
    c_int,
> {
    let prepared = match crate::kernel::task::prepare_exec_image_from_path(path) {
        Ok(p) => p,
        Err(crate::kernel::task::ExecImageError::Fs(FsError::NotFound)) => return Err(-ENOENT),
        Err(crate::kernel::task::ExecImageError::Fs(FsError::IsDirectory)) => return Err(-EISDIR),
        Err(crate::kernel::task::ExecImageError::Fs(_)) => return Err(-EIO),
        Err(crate::kernel::task::ExecImageError::Paging(
            crate::mm::page_table::PagingError::OutOfMemory,
        )) => return Err(-ENOMEM),
        Err(_) => return Err(-ENOEXEC),
    };

    let space = Arc::new(SpinLock::new(prepared.space));
    Ok((
        space,
        prepared.initial_pc,
        prepared.user_sp_high,
        prepared.phdr_addr,
        prepared.phnum,
        prepared.phent,
        prepared.at_base,
        prepared.at_entry,
    ))
}

/// 执行一个新程序（execve）的切换阶段：切换地址空间并恢复到用户态
/// 注意：此函数不会返回！
fn do_execve_switch(
    space: Arc<SpinLock<MemorySpace>>,
    initial_pc: usize,
    sp: usize,
    exe_path: alloc::string::String,
    argv: Vec<alloc::string::String>,
    envp: Vec<alloc::string::String>,
    phdr_addr: usize,
    phnum: usize,
    phent: usize,
    at_base: usize,
    at_entry: usize,
) -> c_int {
    let task = current_task();

    task.lock().fd_table.close_exec();

    // 换掉当前任务的地址空间，e.g. 切换 satp
    {
        let _guard = crate::sync::PreemptGuard::new();
        current_cpu().switch_space(space.clone());
    }

    // 此时在syscall处理的中断上下文中，中断已关闭，直接修改当前任务的trapframe
    // 注意：space 被 clone 进了 execve，所以这里的 space 变量仍然有效
    {
        // 构造 &str 切片供 execve 使用 (Inner scope to ensure borrows end)
        let argv_refs: Vec<&str> = argv.iter().map(|s| s.as_str()).collect();
        let envp_refs: Vec<&str> = envp.iter().map(|s| s.as_str()).collect();

        let mut t = task.lock();
        t.exe_path = Some(exe_path);
        t.execve(
            space.clone(),
            initial_pc,
            sp,
            argv_refs.as_slice(),
            envp_refs.as_slice(),
            phdr_addr,
            phnum,
            phent,
            at_base,
            at_entry,
        );
    } // argv_refs/envp_refs dropped here, ending borrow of argv/envp

    let tfp = task.lock().trap_frame_ptr.load(Ordering::SeqCst);

    // Explicitly drop all owned resources before diverging
    drop(argv);
    drop(envp);
    drop(space); // Drop the Arc<MemorySpace> passed in
    drop(task); // Drop current task ref

    // SAFETY: tfp 指向的内存已经被分配且由当前任务拥有
    // 直接按 trapframe 状态恢复并 sret 到用户态
    unsafe {
        restore(&*tfp);
    }
    -1
}
