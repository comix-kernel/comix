//! IPC 相关的系统调用实现

use alloc::sync::Arc;

use crate::{
    arch::trap::SumGuard,
    kernel::{current_cpu, current_task},
    vfs::{FdFlags, File, FsError, OpenFlags, PipeFile},
};

pub fn dup(oldfd: usize) -> isize {
    let task = current_task();
    match task.lock().fd_table.dup(oldfd) {
        Ok(newfd) => newfd as isize,
        Err(e) => e.to_errno(),
    }
}

pub fn dup3(oldfd: usize, newfd: usize, flags: u32) -> isize {
    let task = current_task();

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

pub fn pipe2(pipefd: *mut i32, flags: u32) -> isize {
    if pipefd.is_null() {
        return FsError::InvalidArgument.to_errno();
    }

    let valid_flags = OpenFlags::O_CLOEXEC | OpenFlags::O_NONBLOCK;
    if flags & !valid_flags.bits() != 0 {
        return FsError::InvalidArgument.to_errno();
    }

    let fd_flags =
        FdFlags::from_open_flags(OpenFlags::from_bits(flags).unwrap_or(OpenFlags::empty()));

    let (pipe_read, pipe_write) = PipeFile::create_pair();

    // 获取当前任务的 FD 表
    let fd_table = current_task().lock().fd_table.clone();

    // 分配文件描述符
    let read_fd =
        match fd_table.alloc_with_flags(Arc::new(pipe_read) as Arc<dyn File>, fd_flags.clone()) {
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
    {
        let _guard = SumGuard::new();
        unsafe {
            core::ptr::write(pipefd.offset(0), read_fd as i32);
            core::ptr::write(pipefd.offset(1), write_fd as i32);
        }
    }

    0
}
