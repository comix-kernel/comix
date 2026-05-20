use super::*;

pub fn close(fd: usize) -> isize {
    let task = current_task();
    let task_lock = task.lock();
    let tid = task_lock.tid as usize;

    // If this fd is a socket, also remove the (tid, fd) -> socket handle mapping.
    // Otherwise, fd reuse can accidentally refer to a stale socket handle.
    if let Ok(file) = task_lock.fd_table.get(fd)
        && file
            .as_any()
            .downcast_ref::<crate::net::socket::SocketFile>()
            .is_some()
    {
        crate::net::socket::unregister_socket_fd(tid, fd);
    }

    match task_lock.fd_table.close(fd) {
        Ok(()) => 0,
        Err(e) => e.to_errno(),
    }
}

pub fn lseek(fd: usize, offset: isize, whence: usize) -> isize {
    // 获取文件对象
    let task = current_task();
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

/// ftruncate - 截断/扩展文件到指定长度
///
/// # 语义（与 Linux 对齐的子集）
/// - 仅支持普通可截断文件（通过 file.inode()->inode.truncate）。
/// - length < 0 返回 EINVAL。
pub fn ftruncate(fd: usize, length: i64) -> isize {
    if length < 0 {
        return FsError::InvalidArgument.to_errno();
    }
    let new_size = length as usize;

    let task = current_task();
    let file = match task.lock().fd_table.get(fd) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    if !file.writable() {
        return FsError::PermissionDenied.to_errno();
    }

    let inode = match file.inode() {
        Ok(i) => i,
        Err(e) => return e.to_errno(),
    };

    match inode.truncate(new_size) {
        Ok(()) => 0,
        Err(e) => e.to_errno(),
    }
}
