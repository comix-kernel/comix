//! 任务相关的系统调用实现

use core::{
    ffi::{c_char, c_int},
    sync::atomic::Ordering,
};

use alloc::{sync::Arc, vec::Vec};
use riscv::register::sstatus;

use crate::{
    arch::trap::restore,
    kernel::{
        SCHEDULER, Scheduler, TASK_MANAGER, TaskManagerTrait, TaskStruct, current_cpu,
        current_task, exit_process, schedule,
        syscall::util::{get_args_safe, get_path_safe},
    },
    mm::{
        frame_allocator::{alloc_contig_frames, alloc_frame},
        memory_space::MemorySpace,
    },
    sync::SpinLock,
    tool::user_buffer::{read_from_user, write_to_user},
    uapi::{
        errno::{ENAVAIL, ESRCH},
        resource::{RLIM_NLIMITS, Rlimit},
    },
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
    let task = current_task();
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
    exit_process(current_task(), code & 0xFF);
    schedule();
    unreachable!("exit: exit_task should not return.");
}

/// 创建当前任务的子任务（fork）
pub fn fork() -> usize {
    let tid = { TASK_MANAGER.lock().allocate_tid() };
    let (ppid, space, signal_handlers, blocked, ptf, fd_table, cwd, root, uts, rlimit) = {
        let cpu = current_cpu().lock();
        let task = cpu.current_task.as_ref().unwrap().lock();
        (
            task.pid,
            task.memory_space
                .as_ref()
                .unwrap()
                .lock()
                .clone_for_fork()
                .expect("fork: clone memory space failed."),
            task.signal_handlers.clone(),
            task.blocked,
            task.trap_frame_ptr.load(Ordering::SeqCst),
            task.fd_table.clone(),
            task.cwd.clone(),
            task.root.clone(),
            task.uts_namespace.clone(),
            task.rlimit.clone(),
        )
    };

    let kstack_tracker = alloc_contig_frames(4).expect("fork: alloc kstack failed.");
    let trap_frame_tracker = alloc_frame().expect("fork: alloc trap frame failed");
    let mut child_task = TaskStruct::utask_create(
        tid,
        tid,
        ppid,
        TaskStruct::empty_children(),
        kstack_tracker,
        trap_frame_tracker,
        Arc::new(SpinLock::new(space)),
        signal_handlers,
        blocked,
        uts,
        rlimit,
    );

    child_task.fd_table = Arc::new(fd_table.clone_table());
    child_task.cwd = cwd;
    child_task.root = root;

    let tf = child_task.trap_frame_ptr.load(Ordering::SeqCst);
    unsafe {
        (*tf).set_fork_trap_frame(&*ptf);
    }
    let child_task = child_task.into_shared();
    current_cpu()
        .lock()
        .current_task
        .as_ref()
        .unwrap()
        .lock()
        .children
        .lock()
        .push(child_task.clone());

    TASK_MANAGER.lock().add_task(child_task.clone());
    SCHEDULER.lock().add_task(child_task);
    tid as usize
}

/// 执行一个新程序（execve）
/// # 参数
/// - `path`: 可执行文件路径
/// - `argv`: 命令行参数
/// - `envp`: 环境变量
pub fn execve(
    path: *const c_char,
    argv: *const *const c_char,
    envp: *const *const c_char,
) -> isize {
    unsafe { sstatus::set_sum() };
    let path_str = get_path_safe(path).unwrap_or("");
    let data = match crate::vfs::vfs_load_elf(path_str) {
        Ok(data) => data,
        Err(_) => return -1,
    };

    // 将 C 风格的 argv/envp (*const *const u8) 转为 Vec<String> / Vec<&str>
    let argv_strings = get_args_safe(argv, "argv").unwrap_or_else(|_| Vec::new());
    let envp_strings = get_args_safe(envp, "envp").unwrap_or_else(|_| Vec::new());
    // 构造 &str 切片（String 的所有权在本函数内，切片在调用 t.execve 时仍然有效）
    let argv_refs: Vec<&str> = argv_strings.iter().map(|s| s.as_str()).collect();
    let envp_refs: Vec<&str> = envp_strings.iter().map(|s| s.as_str()).collect();
    unsafe { sstatus::clear_sum() };
    let task = {
        let cpu = current_cpu().lock();
        cpu.current_task.as_ref().unwrap().clone()
    };

    task.lock().fd_table.close_exec();

    let (space, entry, sp) = MemorySpace::from_elf(&data)
        .expect("kernel_execve: failed to create memory space from ELF");
    let space = Arc::new(SpinLock::new(space));
    // 换掉当前任务的地址空间，e.g. 切换 satp
    current_cpu().lock().switch_space(space.clone());

    // 此时在syscall处理的中断上下文中，中断已关闭，直接修改当前任务的trapframe
    {
        let mut t = task.lock();
        t.execve(space, entry, sp, &argv_refs, &envp_refs);
    }

    let tfp = task.lock().trap_frame_ptr.load(Ordering::SeqCst);
    // SAFETY: tfp 指向的内存已经被分配且由当前任务拥有
    // 直接按 trapframe 状态恢复并 sret 到用户态
    unsafe {
        restore(&*tfp);
    }
    -1
}

/// 等待子进程状态变化
/// TODO: 目前只支持等待退出且只有阻塞模式
pub fn wait(_tid: u32, wstatus: *mut i32, _opt: usize) -> isize {
    // 阻塞当前任务,直到指定的子任务结束
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();

    let (tid, exit_code) = loop {
        let wait_child_ptr = {
            let mut t = task.lock();
            if let Some(res) = t.check_child_exit_locked() {
                break res;
            }
            // 获取 wait_child 的裸指针，以便在释放锁后使用
            // SAFETY: task 是 Arc<SpinLock<Task>>，只要 task 还在，wait_child 就有效
            &mut t.wait_child as *mut crate::kernel::WaitQueue
        };

        // 释放锁后睡眠
        // SAFETY: 我们持有 task 的 Arc，所以 wait_child_ptr 是有效的
        // 虽然有竞争风险（其他核可能同时修改 wait_child），但在 wait 场景下
        // 主要竞争是 wake_up，WaitQueue 内部有自旋锁保护，是安全的
        unsafe {
            (*wait_child_ptr).sleep(task.clone());
        }
    };

    {
        let mut tm = TASK_MANAGER.lock();
        if let Some(child_task) = tm.get_task(tid) {
            tm.release_task(child_task);
        }
    }
    unsafe {
        sstatus::set_sum();
        *wstatus = exit_code;
        sstatus::clear_sum();
    }
    tid as isize
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

/// 获取资源限制
/// # 参数
/// - `resource`: 资源限制 ID
/// - `rlim`: 指向 rlimit 结构体的指针, 用于存储获取到的资源限制
/// # 返回值
/// - 成功返回 0, 失败返回错误码
pub fn getrlimit(resource: c_int, rlim: *mut Rlimit) -> c_int {
    if resource as usize >= RLIM_NLIMITS {
        return -ENAVAIL;
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
/// - 成功返回 0, 失败返回错误码
pub fn setrlimit(resource: c_int, rlim: *const Rlimit) -> c_int {
    if resource as usize >= RLIM_NLIMITS {
        return -ENAVAIL;
    }
    let new_limit = unsafe { read_from_user(rlim) };
    if new_limit.rlim_cur > new_limit.rlim_max {
        return -ENAVAIL;
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
/// - 成功返回 0, 失败返回错误码
pub fn prlimit(
    pid: c_int,
    resource: c_int,
    new_limit: *const Rlimit,
    old_limit: *mut Rlimit,
) -> c_int {
    if resource as usize >= RLIM_NLIMITS {
        return -ENAVAIL;
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
            return -ENAVAIL;
        }
        let rlimit_lock = target_task.lock().rlimit.clone();
        rlimit_lock.lock().limits[resource as usize] = new_rlim;
    }

    0
    // TODO: EPERM, EPERM 和 EFAULT
}
