//! 系统调用模块
//!
//! 提供系统调用的实现
#![allow(dead_code)]

use core::sync::atomic::Ordering;

use alloc::{sync::Arc, vec::Vec};
use riscv::register::sstatus;

use crate::{
    arch::{
        lib::{console::stdin, sbi::console_putchar},
        trap::restore,
    },
    fs::ROOT_FS,
    impl_syscall,
    kernel::{TASK_MANAGER, TaskManagerTrait, TaskStruct, current_cpu, schedule},
    mm::{
        activate,
        frame_allocator::{alloc_contig_frames, alloc_frame},
        memory_space::MemorySpace,
    },
    tool::{copy_cstr_to_string, ptr_array_to_vec_strings},
};

/// 关闭系统调用
fn shutdown() -> ! {
    crate::shutdown(false);
}

/// TODO: 进程退出系统调用
/// # 参数
/// - `code`: 退出代码
fn exit(code: i32) -> ! {
    let (tid, ppid) = {
        let cpu = current_cpu().lock();
        let task = cpu.current_task.as_ref().unwrap().lock();
        (task.tid, task.ppid)
    };
    TASK_MANAGER.lock().exit_task(tid, code);
    TASK_MANAGER
        .lock()
        .get_task(ppid)
        .unwrap()
        .lock()
        .notify_child_exit();
    schedule();
    unreachable!("exit: exit_task should not return.");
}

/// 向文件描述符写入数据
/// # 参数
/// - `fd`: 文件描述符
/// - `buf`: 要写入的数据缓冲区
/// - `count`: 要写入的字节数
fn write(fd: usize, buf: *const u8, count: usize) -> isize {
    if fd == 1 {
        unsafe { sstatus::set_sum() };
        for i in 0..count {
            let c = unsafe { *buf.add(i) };
            console_putchar(c as usize);
        }
        unsafe { sstatus::clear_sum() };
        count as isize
    } else {
        -1 // 不支持其他文件描述符
    }
}

/// 从文件描述符读取数据
/// # 参数
/// - `fd`: 文件描述符
/// - `buf`: 存储读取数据的缓冲区
/// - `count`: 要读取的字节数
fn read(fd: usize, buf: *mut u8, count: usize) -> isize {
    if fd == 0 {
        unsafe { sstatus::set_sum() };
        let mut c = 0;
        while c < count {
            let ch = stdin().read_char();
            unsafe {
                *buf.add(c) = ch as u8;
            }
            c += 1;
        }
        unsafe { sstatus::clear_sum() };
        return c as isize;
    }
    -1 // 不支持其他文件描述符
}

/// 创建当前任务的子任务（fork）
fn fork() -> usize {
    let tid = { TASK_MANAGER.lock().allocate_tid() };
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    let ppid = task.lock().pid;
    let memory_space = task
        .lock()
        .memory_space
        .clone()
        .expect("fork: Can only call fork on user task.")
        .clone_for_fork()
        .expect("fork: clone memory space failed.");
    let kstack_tracker = alloc_contig_frames(4).expect("fork: alloc kstack failed.");
    let trap_frame_tracker = alloc_frame().expect("fork: alloc trap frame failed");
    let child_task = super::TaskStruct::utask_create(
        tid,
        tid,
        ppid,
        TaskStruct::empty_children(),
        kstack_tracker,
        trap_frame_tracker,
        Arc::new(memory_space),
    );
    let tf = child_task.trap_frame_ptr.load(Ordering::SeqCst);
    let ptf = task.lock().trap_frame_ptr.load(Ordering::SeqCst);
    unsafe {
        (*tf).set_fork_trap_frame(&*ptf);
    }
    let child_task = child_task.into_shared();
    task.lock().children.lock().push(child_task);
    tid as usize
}

/// 执行一个新程序（execve）
/// # 参数
/// - `path`: 可执行文件路径
/// - `argv`: 命令行参数
/// - `envp`: 环境变量
fn execve(path: *const u8, argv: *const *const u8, envp: *const *const u8) -> isize {
    let path_str = unsafe {
        match copy_cstr_to_string(path) {
            Ok(s) => s,
            Err(_) => return -1,
        }
    };
    let data_result = crate::vfs::vfs_load_elf(&path_str);

    if data_result.is_err() {
        return -1;
    }
    let data = data_result.unwrap();

    let (space, entry, sp) = MemorySpace::from_elf(&data)
        .expect("kernel_execve: failed to create memory space from ELF");
    let space: Arc<MemorySpace> = Arc::new(space);
    // 换掉当前任务的地址空间，e.g. 切换 satp
    activate(space.root_ppn());

    // 将 C 风格的 argv/envp (*const *const u8) 转为 Vec<String> / Vec<&str>
    let argv_strings = match unsafe { ptr_array_to_vec_strings(argv) } {
        Ok(v) => v,
        Err(_) => return -1,
    };
    let envp_strings = match unsafe { ptr_array_to_vec_strings(envp) } {
        Ok(v) => v,
        Err(_) => return -1,
    };
    // 构造 &str 切片（String 的所有权在本函数内，切片在调用 t.execve 时仍然有效）
    let argv_refs: Vec<&str> = argv_strings.iter().map(|s| s.as_str()).collect();
    let envp_refs: Vec<&str> = envp_strings.iter().map(|s| s.as_str()).collect();

    let task = {
        let cpu = current_cpu().lock();
        cpu.current_task.as_ref().unwrap().clone()
    };
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
fn wait(wstatus: *mut i32) -> isize {
    // 阻塞当前任务，直到指定的子任务结束
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    let (tid, exit_code) = task.lock().wait_for_child();
    unsafe {
        *wstatus = exit_code;
    }
    tid as isize
}

// 系统调用实现注册
impl_syscall!(sys_shutdown, shutdown, noreturn, ());
impl_syscall!(sys_exit, exit, noreturn, (i32));
impl_syscall!(sys_write, write, (usize, *const u8, usize));
impl_syscall!(sys_read, read, (usize, *mut u8, usize));
impl_syscall!(sys_fork, fork, ());
impl_syscall!(
    sys_execve,
    execve,
    (*const u8, *const *const u8, *const *const u8)
);
impl_syscall!(sys_wait, wait, (*mut i32));
