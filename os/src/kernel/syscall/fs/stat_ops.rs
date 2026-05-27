use super::*;

pub fn fstat(fd: usize, statbuf: *mut Stat) -> isize {
    // 检查指针有效性
    if statbuf.is_null() {
        return FsError::InvalidArgument.to_errno();
    }

    // 获取当前任务和文件对象
    let task = current_task();
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
    unsafe { write_to_user(statbuf, stat) };

    0
}

pub fn getdents64(fd: usize, dirp: *mut u8, count: usize) -> isize {
    use crate::vfs::{LinuxDirent64, inode_type_to_d_type};

    // 检查参数有效性
    if dirp.is_null() || count == 0 {
        return FsError::InvalidArgument.to_errno();
    }

    // 获取当前任务和文件对象
    let task = current_task();
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
    // TODO: readdir 返回所有项，对于大目录效率低，且 racey。
    // 应改进为支持从 offset 读取，或者缓存 readdir 结果。
    let entries = match inode.readdir() {
        Ok(e) => e,
        Err(e) => return e.to_errno(),
    };

    // 获取当前文件偏移量 (作为 entry 索引)
    // 假设目录的 offset 就是 entry 的 index
    let start_index = match file.lseek(0, SeekWhence::Cur) {
        Ok(pos) => pos,
        Err(e) => return e.to_errno(),
    };

    // 写入目录项到用户空间
    let mut written = 0usize;
    let mut items_written = 0usize;

    for entry in entries.iter().skip(start_index) {
        // 计算这个 dirent 需要的空间
        let dirent_len = LinuxDirent64::total_len(&entry.name);

        // 检查缓冲区是否还有足够空间
        if written + dirent_len > count {
            break;
        }

        // 计算下一个 entry 的 offset (index + 1)
        let current_off = (start_index + items_written + 1) as i64;

        // 写入 dirent 头部
        unsafe {
            write_to_user(
                dirp.add(written) as *mut LinuxDirent64,
                LinuxDirent64 {
                    d_ino: entry.inode_no as u64,
                    d_off: current_off,
                    d_reclen: dirent_len as u16,
                    d_type: inode_type_to_d_type(entry.inode_type),
                },
            );
        }

        // 写入文件名到 d_name 字段（从 LinuxDirent64 offset 19 开始）
        let name_offset = written + 19;
        let name_bytes = entry.name.as_bytes();
        unsafe {
            crate::arch::ArchImpl::copy_to_user(
                name_bytes.as_ptr(),
                crate::arch::address::UA::from_usize(dirp as usize + name_offset),
                name_bytes.len(),
            )
            .ok();
            write_to_user(dirp.add(name_offset + name_bytes.len()), 0u8);
        }

        written += dirent_len;
        items_written += 1;
    }

    // 更新文件偏移量
    if items_written > 0 {
        // 更新为新的 index
        if let Err(e) = file.lseek((start_index + items_written) as isize, SeekWhence::Set) {
            crate::pr_warn!(
                "[getdents64] failed to update file offset for fd {}: {:?}",
                fd,
                e
            );
            // 即使更新 offset 失败，我们已经写入了数据，返回 written 更合理。
            // 下一次 getdents64 调用可能会重复读取一些条目，但这比丢失数据或为部分成功的读取返回错误要好。
        }
    }

    // 返回写入的字节数
    written as isize
}

pub fn statfs(path: *const c_char, buf: *mut LinuxStatFs) -> isize {
    // 参数校验
    if buf.is_null() {
        return -(EINVAL as isize);
    }

    // 解析路径字符串
    let path_str = match get_path_safe(path as usize) {
        Ok(s) => s,
        Err(e) => return e.to_errno(),
    };

    // 验证路径存在
    if let Err(e) = vfs_lookup(&path_str) {
        return e.to_errno();
    }

    // 通过 MOUNT_TABLE 查找文件系统
    use crate::vfs::MOUNT_TABLE;
    let mount_point = match MOUNT_TABLE.find_mount(&path_str) {
        Some(mp) => mp,
        None => return -(EINVAL as isize),
    };

    // 获取文件系统统计信息
    let fs_stat = match mount_point.fs.statfs() {
        Ok(s) => s,
        Err(e) => return e.to_errno(),
    };

    // 转换为 Linux statfs 结构
    let fs_type = FileSystemType::from_str(mount_point.fs.fs_type());

    let statfs_buf = LinuxStatFs {
        f_type: fs_type.magic(),
        f_bsize: fs_stat.block_size as i64,
        f_blocks: fs_stat.total_blocks as u64,
        f_bfree: fs_stat.free_blocks as u64,
        f_bavail: fs_stat.available_blocks as u64,
        f_files: fs_stat.total_inodes as u64,
        f_ffree: fs_stat.free_inodes as u64,
        f_fsid: [
            (fs_stat.fsid & 0xFFFFFFFF) as i32,
            (fs_stat.fsid >> 32) as i32,
        ],
        f_namelen: fs_stat.max_filename_len as i64,
        f_frsize: fs_stat.block_size as i64, // 片段大小等于块大小
        f_flags: 0,                          // TODO: 添加挂载标志支持
        f_spare: [0; 4],
    };

    // 写回用户空间
    unsafe { write_to_user(buf, statfs_buf) };

    0
}

pub fn faccessat(dirfd: i32, pathname: *const c_char, mode: i32, flags: u32) -> isize {
    // 解析路径字符串
    let path_str = match get_path_safe(pathname as usize) {
        Ok(s) => s,
        Err(e) => return e.to_errno(),
    };

    // 解析标志
    let at_flags = match AtFlags::from_bits(flags) {
        Some(f) => f,
        None => return -(EINVAL as isize),
    };

    // 查找文件
    let follow_symlink = !at_flags.contains(AtFlags::SYMLINK_NOFOLLOW);
    let dentry = match resolve_at_path_with_flags(dirfd, &path_str, follow_symlink) {
        Ok(d) => d,
        Err(e) => return e.to_errno(),
    };

    // 获取文件元数据
    let meta = match dentry.inode.metadata() {
        Ok(m) => m,
        Err(e) => return e.to_errno(),
    };

    // F_OK 模式：仅检查文件是否存在
    if mode == F_OK {
        return 0;
    }

    // 检查权限
    // TODO: 完整的权限检查需要考虑：
    // - 进程的 uid/euid/gid/egid
    // - 文件的 uid/gid/mode
    // - Capabilities (CAP_DAC_OVERRIDE, CAP_DAC_READ_SEARCH)
    // - AT_EACCESS 标志（使用有效 UID 而非实际 UID）
    //
    // 临时实现：简化的权限检查

    // 简化版：只检查文件权限位，假设当前用户有权限
    if (mode & R_OK) != 0 {
        // 检查是否有任何读权限位
        if !meta.mode.contains(FileMode::S_IRUSR)
            && !meta.mode.contains(FileMode::S_IRGRP)
            && !meta.mode.contains(FileMode::S_IROTH)
        {
            return -(EACCES as isize);
        }
    }

    if (mode & W_OK) != 0 {
        // 检查是否有任何写权限位
        if !meta.mode.contains(FileMode::S_IWUSR)
            && !meta.mode.contains(FileMode::S_IWGRP)
            && !meta.mode.contains(FileMode::S_IWOTH)
        {
            return -(EACCES as isize);
        }
    }

    if (mode & X_OK) != 0 {
        // 检查是否有任何执行权限位
        if !meta.mode.contains(FileMode::S_IXUSR)
            && !meta.mode.contains(FileMode::S_IXGRP)
            && !meta.mode.contains(FileMode::S_IXOTH)
        {
            return -(EACCES as isize);
        }
    }

    0
}

pub fn readlinkat(dirfd: i32, pathname: *const c_char, buf: *mut u8, bufsiz: usize) -> isize {
    // 参数校验
    if buf.is_null() || bufsiz == 0 {
        return -(EINVAL as isize);
    }

    // 解析路径字符串
    let path_str = match get_path_safe(pathname as usize) {
        Ok(s) => s,
        Err(e) => return e.to_errno(),
    };

    // 查找符号链接（不跟随最后一级的符号链接）
    let dentry = match resolve_at_path_with_flags(dirfd, &path_str, false) {
        Ok(d) => d,
        Err(e) => return e.to_errno(),
    };

    // 验证是符号链接
    let meta = match dentry.inode.metadata() {
        Ok(m) => m,
        Err(e) => return e.to_errno(),
    };

    if meta.inode_type != InodeType::Symlink {
        return -(EINVAL as isize);
    }

    // 读取符号链接目标（readlink 不依赖 metadata.size，且支持 procfs 等动态符号链接）
    let target = match dentry.inode.readlink() {
        Ok(s) => s,
        Err(e) => return e.to_errno(),
    };
    let bytes_read = core::cmp::min(target.len(), bufsiz);

    // 复制到用户空间（注意：readlink 不添加 null 终止符）
    unsafe {
        crate::arch::ArchImpl::copy_to_user(
            target.as_bytes().as_ptr(),
            crate::arch::address::UA::from_usize(buf as usize),
            bytes_read,
        )
        .ok();
    }

    bytes_read as isize
}

pub fn newfstatat(dirfd: i32, pathname: *const c_char, statbuf: *mut Stat, flags: u32) -> isize {
    // 参数校验
    if statbuf.is_null() {
        return -(EINVAL as isize);
    }

    // 解析路径
    let path_str = match get_path_safe(pathname as usize) {
        Ok(s) => s,
        Err(e) => return e.to_errno(),
    };

    // 解析标志
    let at_flags = match AtFlags::from_bits(flags) {
        Some(f) => f,
        None => return -(EINVAL as isize),
    };

    // 处理 AT_EMPTY_PATH 标志
    if path_str.is_empty() && at_flags.contains(AtFlags::EMPTY_PATH) {
        if dirfd == AT_FDCWD {
            return -(EINVAL as isize);
        }
        // 对 dirfd 执行 fstat
        return fstat(dirfd as usize, statbuf);
    }

    // 查找文件
    let follow_symlink = !at_flags.contains(AtFlags::SYMLINK_NOFOLLOW);
    let dentry = match resolve_at_path_with_flags(dirfd, &path_str, follow_symlink) {
        Ok(d) => d,
        Err(e) => return e.to_errno(),
    };

    // 获取文件元数据
    let metadata = match dentry.inode.metadata() {
        Ok(m) => m,
        Err(e) => return e.to_errno(),
    };

    // 转换为 Stat 结构
    let stat = Stat::from_metadata(&metadata);

    // 写回用户空间
    unsafe { write_to_user(statbuf, stat) };

    0
}

pub fn statx(
    dirfd: i32,
    pathname: *const c_char,
    flags: u32,
    _mask: u32,
    statxbuf: *mut Statx,
) -> isize {
    if statxbuf.is_null() {
        return -(EINVAL as isize);
    }

    // 解析路径
    let path_str = match get_path_safe(pathname as usize) {
        Ok(s) => s,
        Err(e) => return e.to_errno(),
    };

    let at_flags = AtFlags::from_bits_truncate(flags);

    // 处理 AT_EMPTY_PATH 标志：pathname 为空时，对 dirfd 指向的文件执行 statx
    if path_str.is_empty() && at_flags.contains(AtFlags::EMPTY_PATH) {
        if dirfd == AT_FDCWD {
            return -(EINVAL as isize);
        }

        let task = current_task();
        let file = match task.lock().fd_table.get(dirfd as usize) {
            Ok(f) => f,
            Err(e) => return e.to_errno(),
        };
        let metadata = match file.metadata() {
            Ok(m) => m,
            Err(e) => return e.to_errno(),
        };

        let stx = crate::vfs::Statx::from_metadata(&metadata);
        unsafe { write_to_user(statxbuf, stx) };
        return 0;
    }

    // 查找文件
    let follow_symlink = !at_flags.contains(AtFlags::SYMLINK_NOFOLLOW);
    let dentry = match resolve_at_path_with_flags(dirfd, &path_str, follow_symlink) {
        Ok(d) => d,
        Err(e) => return e.to_errno(),
    };

    let metadata = match dentry.inode.metadata() {
        Ok(m) => m,
        Err(e) => return e.to_errno(),
    };

    let stx = crate::vfs::Statx::from_metadata(&metadata);

    // 写回用户空间
    unsafe { write_to_user(statxbuf, stx) };

    0
}

pub fn utimensat(dirfd: i32, pathname: *const c_char, times: *const TimeSpec, flags: u32) -> isize {
    // 解析路径
    let path_str = match get_path_safe(pathname as usize) {
        Ok(s) => s,
        Err(e) => return e.to_errno(),
    };

    // 解析标志
    let at_flags = match AtFlags::from_bits(flags) {
        Some(f) => f,
        None => return -(EINVAL as isize),
    };

    // 查找文件
    let follow_symlink = !at_flags.contains(AtFlags::SYMLINK_NOFOLLOW);
    let dentry = match resolve_at_path_with_flags(dirfd, &path_str, follow_symlink) {
        Ok(d) => d,
        Err(e) => return e.to_errno(),
    };

    // 解析时间参数
    let (atime_opt, mtime_opt) = if times.is_null() {
        // NULL 表示将两个时间都设置为当前时间
        let now = TimeSpec::now();
        (Some(now), Some(now))
    } else {
        unsafe {
            use crate::util::user_buffer::read_from_user;
            let ts0: TimeSpec = read_from_user(times);
            let ts1: TimeSpec = read_from_user(times.add(1));

            if let Err(e) = ts0.validate() {
                return -(e as isize);
            }
            if let Err(e) = ts1.validate() {
                return -(e as isize);
            }

            // 处理访问时间
            let atime_opt = if ts0.is_omit() {
                None
            } else if ts0.is_now() {
                Some(TimeSpec::now())
            } else {
                Some(ts0)
            };

            // 处理修改时间
            let mtime_opt = if ts1.is_omit() {
                None
            } else if ts1.is_now() {
                Some(TimeSpec::now())
            } else {
                Some(ts1)
            };

            (atime_opt, mtime_opt)
        }
    };

    // 设置时间戳
    if let Err(e) = dentry.inode.set_times(atime_opt, mtime_opt) {
        return e.to_errno();
    }

    0
}
