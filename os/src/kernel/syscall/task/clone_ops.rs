use super::*;

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
        (*tf).set_clone_trap_frame(&*ptf, child_task.kstack_base.as_usize(), stack as usize);
        if requested_flags.contains(CloneFlags::SETTLS) {
            (*tf).set_tls(tls as usize);
        }
    }
    if requested_flags.contains(CloneFlags::CHILD_SETTID) {
        child_task.set_child_tid = Some(UA::from_usize(ctid as usize));
    }
    if requested_flags.contains(CloneFlags::CHILD_CLEARTID) {
        child_task.clear_child_tid = Some(UA::from_usize(ctid as usize));
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
    let current_cpu = crate::arch::cpu_id();
    if target_cpu != current_cpu {
        crate::pr_debug!(
            "[SMP] Sending IPI from CPU {} to CPU {}",
            current_cpu,
            target_cpu
        );
        crate::arch::send_reschedule_ipi(target_cpu);
    }

    tid as c_int
}
