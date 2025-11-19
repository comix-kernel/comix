//! 系统调用模块
//!
//! 提供系统调用的实现
#![allow(dead_code)]

use core::{
    ffi::{CStr, c_char},
    sync::atomic::Ordering,
};

use alloc::{
    format,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use riscv::register::sstatus;

use crate::{
    arch::{lib::sbi, trap::restore},
    // fs::ROOT_FS,
    impl_syscall,
    kernel::{
        SCHEDULER, Scheduler, TASK_MANAGER, TaskManagerTrait, TaskStruct, current_cpu,
        current_task, do_exit, schedule,
    },
    mm::{
        activate,
        frame_allocator::{alloc_contig_frames, alloc_frame},
        memory_space::MemorySpace,
    },
    sync::SpinLock,
    vfs::{
        DENTRY_CACHE, Dentry, DiskFile, FDFlags, File, FileMode, FsError, InodeType, LinuxDirent64,
        OpenFlags, PipeFile, SeekWhence, Stat, dentry, get_root_dentry, inode_type_to_d_type,
        split_path, vfs_lookup, vfs_lookup_from,
    },
};

const AT_FDCWD: i32 = -100;
const AT_SYMLINK_NOFOLLOW: u32 = 0x100;
const AT_REMOVEDIR: u32 = 0x200;
const O_CLOEXEC: u32 = 0o2000000;

/// 关闭系统调用
fn shutdown() -> ! {
    crate::shutdown(false);
}

/// TODO: 进程退出系统调用
/// # 参数
/// - `code`: 退出代码
fn exit(code: i32) -> ! {
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    do_exit(task, code);
    schedule();
    unreachable!("exit: exit_task should not return.");
}

/// 向文件描述符写入数据
/// # 参数
/// - `fd`: 文件描述符
/// - `buf`: 要写入的数据缓冲区
/// - `count`: 要写入的字节数
fn write(fd: usize, buf: *const u8, count: usize) -> isize {
    // 1. 获取文件对象
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    let file = match task.lock().fd_table.get(fd) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    // 2. 访问用户态缓冲区
    unsafe { sstatus::set_sum() };
    let buffer = unsafe { core::slice::from_raw_parts(buf, count) };

    // 3. 调用File::write（会自动处理O_APPEND和offset）
    let result = match file.write(buffer) {
        Ok(n) => n as isize,
        Err(e) => e.to_errno(),
    };

    unsafe { sstatus::clear_sum() };
    result
}

/// 从文件描述符读取数据
/// # 参数
/// - `fd`: 文件描述符
/// - `buf`: 存储读取数据的缓冲区
/// - `count`: 要读取的字节数
fn read(fd: usize, buf: *mut u8, count: usize) -> isize {
    // 1. 获取文件对象
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    let file = match task.lock().fd_table.get(fd) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    // 2. 访问用户态缓冲区
    unsafe { sstatus::set_sum() };
    let buffer = unsafe { core::slice::from_raw_parts_mut(buf, count) };

    // 3. 调用File::read（会自动更新offset）
    let result = match file.read(buffer) {
        Ok(n) => n as isize,
        Err(e) => e.to_errno(),
    };

    unsafe { sstatus::clear_sum() };
    result
}

/// 创建当前任务的子任务（fork）
fn fork() -> usize {
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
    let mut child_task = super::TaskStruct::utask_create(
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
fn execve(path: *const c_char, argv: *const *const c_char, envp: *const *const c_char) -> isize {
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
    activate(space.lock().root_ppn());

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
fn wait(_tid: u32, wstatus: *mut i32, _opt: usize) -> isize {
    // 阻塞当前任务,直到指定的子任务结束
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    let (tid, exit_code) = task.lock().wait_for_child();
    TASK_MANAGER.lock().release_task(tid);
    unsafe {
        sstatus::set_sum();
        *wstatus = exit_code;
        sstatus::clear_sum();
    }
    tid as isize
}

fn close(fd: usize) -> isize {
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    match task.lock().fd_table.close(fd) {
        Ok(()) => 0,
        Err(e) => e.to_errno(),
    }
}

fn lseek(fd: usize, offset: isize, whence: usize) -> isize {
    // 获取文件对象
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    let file = match task.lock().fd_table.get(fd) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    // 转换whence参数
    let seek_whence = match SeekWhence::from_usize(whence) {
        Some(w) => w,
        None => return FsError::InvalidArgument.to_errno(),
    };

    // 执行lseek
    match file.lseek(offset, seek_whence) {
        Ok(new_pos) => new_pos as isize,
        Err(e) => e.to_errno(),
    }
}

/// openat - 相对于目录文件描述符打开文件
fn openat(dirfd: i32, pathname: *const c_char, flags: u32, mode: u32) -> isize {
    // 解析路径字符串
    unsafe { sstatus::set_sum() };
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
        Err(_) => {
            unsafe { sstatus::clear_sum() };
            return FsError::InvalidArgument.to_errno();
        }
    };
    unsafe { sstatus::clear_sum() };

    // 2. 解析标志位
    let open_flags = match OpenFlags::from_bits(flags) {
        Some(f) => f,
        None => return FsError::InvalidArgument.to_errno(),
    };

    // 解析路径（处理AT_FDCWD和相对路径）
    let dentry = match resolve_at_path(dirfd, &path_str) {
        Ok(Some(d)) => {
            // 文件已存在
            // 检查 O_EXCL (与 O_CREAT 一起使用时，文件必须不存在)
            if open_flags.contains(OpenFlags::O_CREAT) && open_flags.contains(OpenFlags::O_EXCL) {
                return FsError::AlreadyExists.to_errno();
            }
            d
        }
        Ok(None) => {
            // 文件不存在，检查是否需要创建
            if !open_flags.contains(OpenFlags::O_CREAT) {
                return FsError::NotFound.to_errno();
            }

            // 创建新文件
            match create_file_at(dirfd, &path_str, mode) {
                Ok(d) => d,
                Err(e) => return e.to_errno(),
            }
        }
        Err(e) => return e.to_errno(),
    };

    // 获取文件元数据
    let meta = match dentry.inode.metadata() {
        Ok(m) => m,
        Err(e) => return e.to_errno(),
    };

    // 检查 O_DIRECTORY (必须是目录)
    if open_flags.contains(OpenFlags::O_DIRECTORY) {
        if meta.inode_type != InodeType::Directory {
            return FsError::NotDirectory.to_errno();
        }
    }

    // 处理 O_TRUNC (截断文件)
    if open_flags.contains(OpenFlags::O_TRUNC) && open_flags.writable() {
        if meta.inode_type == InodeType::File {
            if let Err(e) = dentry.inode.truncate(0) {
                return e.to_errno();
            }
        }
    }

    // 创建 File 对象
    let file = Arc::new(DiskFile::new(dentry, open_flags));

    // 分配文件描述符
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    match task.lock().fd_table.alloc(file) {
        Ok(fd) => fd as isize,
        Err(e) => e.to_errno(),
    }
}

fn mkdirat(dirfd: i32, pathname: *const c_char, mode: u32) -> isize {
    // 解析路径
    unsafe { sstatus::set_sum() };
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
        Err(_) => {
            unsafe { sstatus::clear_sum() };
            return FsError::InvalidArgument.to_errno();
        }
    };
    unsafe { sstatus::clear_sum() };

    // 分割路径为目录和文件名
    let (dir_path, dirname) = match split_path(&path_str) {
        Ok(p) => p,
        Err(e) => return e.to_errno(),
    };

    // 查找父目录
    let parent_dentry = match resolve_at_path(dirfd, &dir_path) {
        Ok(Some(d)) => d,
        Ok(None) => return FsError::NotFound.to_errno(),
        Err(e) => return e.to_errno(),
    };

    // 创建目录
    let dir_mode = FileMode::from_bits_truncate(mode) | FileMode::S_IFDIR;
    match parent_dentry.inode.mkdir(&dirname, dir_mode) {
        Ok(_) => 0,
        Err(e) => e.to_errno(),
    }
}

fn unlinkat(dirfd: i32, pathname: *const c_char, flags: u32) -> isize {
    // 解析路径
    unsafe { sstatus::set_sum() };
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
        Err(_) => {
            unsafe { sstatus::clear_sum() };
            return FsError::InvalidArgument.to_errno();
        }
    };
    unsafe { sstatus::clear_sum() };

    let is_rmdir = (flags & AT_REMOVEDIR) != 0;

    // 分割路径
    let (dir_path, filename) = match split_path(&path_str) {
        Ok(p) => p,
        Err(e) => return e.to_errno(),
    };

    // 查找父目录
    let parent_dentry = match resolve_at_path(dirfd, &dir_path) {
        Ok(Some(d)) => d,
        Ok(None) => return FsError::NotFound.to_errno(),
        Err(e) => return e.to_errno(),
    };

    // 检查目标文件类型
    let target_inode = match parent_dentry.inode.lookup(&filename) {
        Ok(i) => i,
        Err(e) => return e.to_errno(),
    };

    let meta = match target_inode.metadata() {
        Ok(m) => m,
        Err(e) => return e.to_errno(),
    };

    // 验证文件类型与flags匹配
    if is_rmdir {
        // rmdir: 必须是目录
        if meta.inode_type != InodeType::Directory {
            return FsError::NotDirectory.to_errno();
        }
    } else {
        // unlink: 不能是目录
        if meta.inode_type == InodeType::Directory {
            return FsError::IsDirectory.to_errno();
        }
    }

    // 删除目录项
    match parent_dentry.inode.unlink(&filename) {
        Ok(()) => {
            // 从缓存中移除
            parent_dentry.remove_child(&filename);
            0
        }
        Err(e) => e.to_errno(),
    }
}

fn chdir(path: *const c_char) -> isize {
    // 解析路径
    unsafe { sstatus::set_sum() };
    let path_str = match get_path_safe(path) {
        Ok(s) => s,
        Err(_) => {
            unsafe { sstatus::clear_sum() };
            return FsError::InvalidArgument.to_errno();
        }
    };
    unsafe { sstatus::clear_sum() };

    // 查找目标目录
    let dentry = match vfs_lookup(path_str) {
        Ok(d) => d,
        Err(e) => return e.to_errno(),
    };

    // 检查是否为目录
    let meta = match dentry.inode.metadata() {
        Ok(m) => m,
        Err(e) => return e.to_errno(),
    };

    if meta.inode_type != InodeType::Directory {
        return FsError::NotDirectory.to_errno();
    }

    // 更新当前工作目录
    current_task().lock().cwd = Some(dentry);
    0
}

fn getcwd(buf: *mut u8, size: usize) -> isize {
    // 获取当前工作目录dentry
    let cwd_dentry = match current_task().lock().cwd.clone() {
        Some(d) => d,
        None => return FsError::NotSupported.to_errno(),
    };

    // 获取完整路径
    let path = cwd_dentry.full_path();
    let path_bytes = path.as_bytes();

    // 检查缓冲区大小
    if path_bytes.len() + 1 > size {
        return FsError::InvalidArgument.to_errno();
    }

    // 复制到用户态缓冲区
    unsafe {
        sstatus::set_sum();
        core::ptr::copy_nonoverlapping(path_bytes.as_ptr(), buf, path_bytes.len());
        *buf.add(path_bytes.len()) = 0; // null terminator
        sstatus::clear_sum();
    }

    buf as isize
}

fn dup(oldfd: usize) -> isize {
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    match task.lock().fd_table.dup(oldfd) {
        Ok(newfd) => newfd as isize,
        Err(e) => e.to_errno(),
    }
}

fn dup3(oldfd: usize, newfd: usize, flags: u32) -> isize {
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();

    let open_flags = match OpenFlags::from_bits(flags) {
        Some(f) => f,
        None => return FsError::InvalidArgument.to_errno(),
    };

    if open_flags.bits() & !OpenFlags::O_CLOEXEC.bits() != 0 {
        return FsError::InvalidArgument.to_errno();
    }

    match task.lock().fd_table.dup3(oldfd, newfd, open_flags) {
        Ok(newfd) => newfd as isize,
        Err(e) => e.to_errno(),
    }
}

fn pipe2(pipefd: *mut i32, flags: u32) -> isize {
    if pipefd.is_null() {
        return FsError::InvalidArgument.to_errno();
    }

    let valid_flags = OpenFlags::O_CLOEXEC | OpenFlags::O_NONBLOCK;
    if flags & !valid_flags.bits() != 0 {
        return FsError::InvalidArgument.to_errno();
    }

    let fd_flags =
        FDFlags::from_open_flags(OpenFlags::from_bits(flags).unwrap_or(OpenFlags::empty()));

    let (pipe_read, pipe_write) = PipeFile::create_pair();

    // 获取当前任务的 FD 表
    let fd_table = current_task().lock().fd_table.clone();

    // 分配文件描述符
    let read_fd = match fd_table.alloc_with_flags(Arc::new(pipe_read) as Arc<dyn File>, fd_flags.clone()) {
        Ok(fd) => fd,
        Err(e) => return e.to_errno(),
    };

    let write_fd = match fd_table.alloc_with_flags(Arc::new(pipe_write) as Arc<dyn File>, fd_flags)
    {
        Ok(fd) => fd,
        Err(e) => {
            // 分配失败，需要回滚读端 FD
            let _ = fd_table.close(read_fd);
            return e.to_errno();
        }
    };

    // 将 FD 写回用户空间
    unsafe {
        sstatus::set_sum();
        core::ptr::write(pipefd.offset(0), read_fd as i32);
        core::ptr::write(pipefd.offset(1), write_fd as i32);
        sstatus::clear_sum();
    }

    0
}

fn fstat(fd: usize, statbuf: *mut Stat) -> isize {
    // 检查指针有效性
    if statbuf.is_null() {
        return FsError::InvalidArgument.to_errno();
    }

    // 获取当前任务和文件对象
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    let file = match task.lock().fd_table.get(fd) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    // 获取文件元数据
    let metadata = match file.metadata() {
        Ok(m) => m,
        Err(e) => return e.to_errno(),
    };

    // 转换为 Stat 结构
    let stat = crate::vfs::Stat::from_metadata(&metadata);

    // 写回用户空间
    unsafe {
        sstatus::set_sum();
        core::ptr::write(statbuf, stat);
        sstatus::clear_sum();
    }

    0
}

fn getdents64(fd: usize, dirp: *mut u8, count: usize) -> isize {
    use crate::vfs::{LinuxDirent64, inode_type_to_d_type};

    // 检查参数有效性
    if dirp.is_null() || count == 0 {
        return FsError::InvalidArgument.to_errno();
    }

    // 获取当前任务和文件对象
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    let file = match task.lock().fd_table.get(fd) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    // 获取 inode（目录必须通过 inode 读取）
    let inode = match file.inode() {
        Ok(i) => i,
        Err(e) => return e.to_errno(),
    };

    // 读取目录项
    let entries = match inode.readdir() {
        Ok(e) => e,
        Err(e) => return e.to_errno(),
    };

    // 写入目录项到用户空间
    let mut written = 0usize;
    let mut offset = 0i64;

    unsafe {
        sstatus::set_sum();

        for entry in entries {
            // 计算这个 dirent 需要的空间
            let dirent_len = LinuxDirent64::total_len(&entry.name);

            // 检查缓冲区是否还有足够空间
            if written + dirent_len > count {
                break;
            }

            // 写入 dirent 头部
            let dirent_ptr = dirp.add(written) as *mut LinuxDirent64;
            core::ptr::write(
                dirent_ptr,
                LinuxDirent64 {
                    d_ino: entry.inode_no as u64,
                    d_off: offset + dirent_len as i64,
                    d_reclen: dirent_len as u16,
                    d_type: inode_type_to_d_type(entry.inode_type),
                },
            );

            // 写入文件名（在 dirent 结构体之后）
            let name_ptr = dirp.add(written + core::mem::size_of::<LinuxDirent64>());
            let name_bytes = entry.name.as_bytes();
            core::ptr::copy_nonoverlapping(name_bytes.as_ptr(), name_ptr, name_bytes.len());
            // 添加 null 终止符
            core::ptr::write(name_ptr.add(name_bytes.len()), 0);

            written += dirent_len;
            offset += dirent_len as i64;
        }

        sstatus::clear_sum();
    }

    // 返回写入的字节数
    written as isize
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
impl_syscall!(sys_wait, wait, (u32, *mut i32, usize));
impl_syscall!(sys_close, close, (usize));
impl_syscall!(sys_lseek, lseek, (usize, isize, usize));
impl_syscall!(sys_openat, openat, (i32, *const c_char, u32, u32));
impl_syscall!(sys_dup, dup, (usize));
impl_syscall!(sys_dup3, dup3, (usize, usize, u32));
impl_syscall!(sys_pipe2, pipe2, (*mut i32, u32));
impl_syscall!(sys_fstat, fstat, (usize, *mut Stat));
impl_syscall!(sys_getdents64, getdents64, (usize, *mut u8, usize));

fn get_path_safe(path: *const c_char) -> Result<&'static str, &'static str> {
    // 必须在 unsafe 块中进行，因为依赖 C 的正确性
    let c_str = unsafe {
        // 检查指针是否为 NULL (空指针)
        if path.is_null() {
            return Err("Path pointer is NULL");
        }
        // 转换为安全的 &CStr 引用。如果指针无效或非空终止，这里会发生未定义行为 (UB)
        CStr::from_ptr(path)
    };

    // 转换为 Rust 的 &str。to_str() 会检查 UTF-8 有效性
    match c_str.to_str() {
        Ok(s) => Ok(s),
        Err(_) => Err("Path is not valid UTF-8"),
    }
}

fn get_args_safe(
    ptr_array: *const *const c_char,
    name: &str, // 用于错误报告
) -> Result<Vec<String>, String> {
    let mut args = Vec::new();

    // 1. 检查指针数组是否为 NULL
    if ptr_array.is_null() {
        return Ok(Vec::new()); // 可能是合法的空列表
    }

    // 必须在 unsafe 块中进行，因为涉及到裸指针操作
    unsafe {
        let mut current_ptr = ptr_array;

        // 2. 迭代直到遇到 NULL 指针
        while !(*current_ptr).is_null() {
            let c_str = {
                // 3. 将当前的 *const c_char 转换为 &CStr
                CStr::from_ptr(*current_ptr)
            };

            // 4. 转换为 Rust String 并收集
            match c_str.to_str() {
                Ok(s) => args.push(s.to_string()),
                Err(_) => {
                    return Err(format!("{} contains non-UTF-8 string", name));
                }
            }

            // 移动到数组的下一个元素
            current_ptr = current_ptr.add(1);
        }
    }

    Ok(args)
}

/// 解析at系列系统调用的路径
///
/// 这是系统调用层的辅助函数，处理 AT_FDCWD 和相对路径逻辑
fn resolve_at_path(dirfd: i32, path: &str) -> Result<Option<Arc<Dentry>>, FsError> {
    let base_dentry = if path.starts_with('/') {
        get_root_dentry()?
    } else if dirfd == AT_FDCWD {
        current_task()
            .lock()
            .cwd
            .clone()
            .ok_or(FsError::NotSupported)?
    } else {
        // 对于文件描述符，我们需要获取对应的 dentry
        let task = current_task();
        let file = task.lock().fd_table.get(dirfd as usize)?;

        // 验证是目录
        let meta = file.metadata()?;
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        if let Ok(dentry) = file.dentry() {
            dentry
        } else {
            return Err(FsError::NotDirectory);
        }
    };

    match vfs_lookup_from(base_dentry, path) {
        Ok(d) => Ok(Some(d)),
        Err(FsError::NotFound) => Ok(None),
        Err(e) => Err(e),
    }
}

fn create_file_at(dirfd: i32, path: &str, mode: u32) -> Result<Arc<Dentry>, FsError> {
    let (dir_path, filename) = split_path(path)?;
    let parent_dentry = match resolve_at_path(dirfd, &dir_path)? {
        Some(d) => d,
        None => return Err(FsError::NotFound),
    };

    let meta = parent_dentry.inode.metadata()?;
    if meta.inode_type != InodeType::Directory {
        return Err(FsError::NotDirectory);
    }

    let file_mode = FileMode::from_bits_truncate(mode) | FileMode::S_IFREG;
    let child_inode = parent_dentry.inode.create(&filename, file_mode)?;

    let child_dentry = Dentry::new(filename.clone(), child_inode);
    parent_dentry.add_child(child_dentry.clone());
    DENTRY_CACHE.insert(&child_dentry);

    Ok(child_dentry)
}
