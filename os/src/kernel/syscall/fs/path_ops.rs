use super::*;

/// openat - 相对于目录文件描述符打开文件
pub fn openat(dirfd: i32, pathname: *const c_char, flags: u32, mode: u32) -> isize {
    // 解析路径字符串
    let path_str = match get_path_safe(pathname as usize) {
        Ok(s) => s,
        Err(_) => {
            return FsError::InvalidArgument.to_errno();
        }
    };

    // 解析标志位
    let open_flags = match OpenFlags::from_bits(flags) {
        Some(f) => f,
        None => {
            return FsError::InvalidArgument.to_errno();
        }
    };

    // crate::println!("[openat] path: {}, flags: {:?} (raw: 0x{:x})", path_str, open_flags, flags);

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
    if open_flags.contains(OpenFlags::O_DIRECTORY) && meta.inode_type != InodeType::Directory {
        return FsError::NotDirectory.to_errno();
    }

    // 处理 O_TRUNC (截断文件)
    if open_flags.contains(OpenFlags::O_TRUNC)
        && open_flags.writable()
        && meta.inode_type == InodeType::File
        && let Err(e) = dentry.inode.truncate(0)
    {
        return e.to_errno();
    }

    // 创建 File 对象
    let file = match create_file_from_dentry(dentry, open_flags) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    // 分配文件描述符
    let task = current_task();
    match task.lock().fd_table.alloc(file) {
        Ok(fd) => fd as isize,
        Err(e) => e.to_errno(),
    }
}

pub fn mkdirat(dirfd: i32, pathname: *const c_char, mode: u32) -> isize {
    // 解析路径
    let path_str = match get_path_safe(pathname as usize) {
        Ok(s) => s,
        Err(_) => {
            return FsError::InvalidArgument.to_errno();
        }
    };

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

pub fn unlinkat(dirfd: i32, pathname: *const c_char, flags: u32) -> isize {
    // 解析路径
    let path_str = match get_path_safe(pathname as usize) {
        Ok(s) => s,
        Err(_) => {
            return FsError::InvalidArgument.to_errno();
        }
    };

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

pub fn chdir(path: *const c_char) -> isize {
    // 解析路径
    let path_str = match get_path_safe(path as usize) {
        Ok(s) => s,
        Err(_) => {
            return FsError::InvalidArgument.to_errno();
        }
    };

    // 查找目标目录
    let dentry = match vfs_lookup(&path_str) {
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
    current_task().lock().fs.lock().cwd = Some(dentry);
    0
}

pub fn getcwd(buf: *mut u8, size: usize) -> isize {
    // 获取当前工作目录dentry
    let cwd_dentry = match current_task().lock().fs.lock().cwd.clone() {
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
        crate::arch::ArchImpl::copy_to_user(
            path_bytes.as_ptr(),
            crate::arch::address::UA::from_usize(buf as usize),
            path_bytes.len(),
        )
        .ok();
        write_to_user(buf.add(path_bytes.len()), 0u8);
    }

    buf as isize
}
