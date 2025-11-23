//! 任务相关的系统调用实现

use core::{ffi::c_char, sync::atomic::Ordering};

use alloc::{sync::Arc, vec::Vec};
use riscv::register::sstatus;

use crate::{
    arch::trap::restore,
    kernel::{
        SCHEDULER, Scheduler, TASK_MANAGER, TaskManagerTrait, TaskStruct, current_cpu,
        exit_process, schedule,
        syscall::util::{get_args_safe, get_path_safe},
    },
    mm::{
        frame_allocator::{alloc_contig_frames, alloc_frame},
        memory_space::MemorySpace,
    },
    sync::SpinLock,
};

/// 进程退出系统调用
/// # 参数
/// - `code`: 退出代码
pub fn exit(code: i32) -> ! {
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    exit_process(task, code);
    schedule();
    unreachable!("exit: exit_task should not return.");
}

/// 创建当前任务的子任务（fork）
pub fn fork() -> usize {
    let tid = { TASK_MANAGER.lock().allocate_tid() };
    let (ppid, space, signal_handlers, blocked, ptf, fd_table, cwd, root) = {
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
    let (tid, exit_code) = task.lock().wait_for_child();
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
