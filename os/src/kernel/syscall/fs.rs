//! 文件系统相关的系统调用实现

use core::ffi::c_char;

use alloc::{string::ToString, sync::Arc};

use crate::{
    arch::trap::SumGuard,
    kernel::{
        current_cpu, current_task,
        syscall::util::{
            create_file_at, create_file_from_dentry, get_path_safe, resolve_at_path,
            resolve_at_path_with_flags,
        },
    },
    uapi::{
        errno::{EACCES, EINVAL, ENOENT},
        fs::{AtFlags, F_OK, FileSystemType, LinuxStatFs, R_OK, W_OK, X_OK},
        time::TimeSpec,
    },
    vfs::{
        DENTRY_CACHE, Dentry, FileMode, FsError, InodeType, OpenFlags, RegFile, SeekWhence, Stat,
        split_path, vfs_lookup,
    },
};

pub const AT_FDCWD: i32 = -100;
pub const AT_SYMLINK_NOFOLLOW: u32 = 0x100;
pub const AT_REMOVEDIR: u32 = 0x200;
pub const O_CLOEXEC: u32 = 0o2000000;

pub fn close(fd: usize) -> isize {
    let task = current_task();
    match task.lock().fd_table.close(fd) {
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

/// openat - 相对于目录文件描述符打开文件
pub fn openat(dirfd: i32, pathname: *const c_char, flags: u32, mode: u32) -> isize {
    // 解析路径字符串
    let _guard = SumGuard::new();
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
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
    let _guard = SumGuard::new();
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
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
    let _guard = SumGuard::new();
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
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
    let _guard = SumGuard::new();
    let path_str = match get_path_safe(path) {
        Ok(s) => s.to_string(),
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
    {
        let _guard = SumGuard::new();
        unsafe {
            core::ptr::copy_nonoverlapping(path_bytes.as_ptr(), buf, path_bytes.len());
            *buf.add(path_bytes.len()) = 0; // null terminator
        }
    }

    buf as isize
}

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
    {
        let _guard = SumGuard::new();
        unsafe {
            core::ptr::write(statbuf, stat);
        }
    }

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

    unsafe {
        let _guard = SumGuard::new();

        for (_, entry) in entries.iter().skip(start_index).enumerate() {
            // 计算这个 dirent 需要的空间
            let dirent_len = LinuxDirent64::total_len(&entry.name);

            // 检查缓冲区是否还有足够空间
            if written + dirent_len > count {
                break;
            }

            // 计算下一个 entry 的 offset (index + 1)
            let current_off = (start_index + items_written + 1) as i64;

            // 写入 dirent 头部
            let dirent_ptr = dirp.add(written) as *mut LinuxDirent64;
            core::ptr::write(
                dirent_ptr,
                LinuxDirent64 {
                    d_ino: entry.inode_no as u64,
                    d_off: current_off,
                    d_reclen: dirent_len as u16,
                    d_type: inode_type_to_d_type(entry.inode_type),
                },
            );

            // 写入文件名
            // 注意：LinuxDirent64 在 Rust 中由 padding (到 24 字节)
            // 但 d_name 实际上应该从 offset 19 开始 (u64+u64+u16+u8 = 8+8+2+1 = 19)
            // 我们必须覆盖 padding 区域
            let name_ptr = dirp.add(written + 19);
            let name_bytes = entry.name.as_bytes();
            core::ptr::copy_nonoverlapping(name_bytes.as_ptr(), name_ptr, name_bytes.len());
            // 添加 null 终止符
            core::ptr::write(name_ptr.add(name_bytes.len()), 0);

            written += dirent_len;
            items_written += 1;
        }
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
    let _guard = SumGuard::new();
    let path_str = match get_path_safe(path) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return -(EINVAL as isize);
        }
    };

    // 验证路径存在
    if vfs_lookup(&path_str).is_err() {
        return -(EINVAL as isize);
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
    {
        let _guard = SumGuard::new();
        unsafe {
            core::ptr::write(buf, statfs_buf);
        }
    }

    0
}

pub fn faccessat(dirfd: i32, pathname: *const c_char, mode: i32, flags: u32) -> isize {
    // 解析路径字符串
    let _guard = SumGuard::new();
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return -(EINVAL as isize);
        }
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
    let _guard = SumGuard::new();
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return -(EINVAL as isize);
        }
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
    let bytes_read = core::cmp::min(target.as_bytes().len(), bufsiz);

    // 复制到用户空间（注意：readlink 不添加 null 终止符）
    {
        let _guard = SumGuard::new();
        unsafe {
            core::ptr::copy_nonoverlapping(target.as_bytes().as_ptr(), buf, bytes_read);
        }
    }

    bytes_read as isize
}

pub fn newfstatat(dirfd: i32, pathname: *const c_char, statbuf: *mut Stat, flags: u32) -> isize {
    // 参数校验
    if statbuf.is_null() {
        return -(EINVAL as isize);
    }

    // 解析路径
    let _guard = SumGuard::new();
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return -(EINVAL as isize);
        }
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
    {
        let _guard = SumGuard::new();
        unsafe {
            core::ptr::write(statbuf, stat);
        }
    }

    0
}

pub fn utimensat(dirfd: i32, pathname: *const c_char, times: *const TimeSpec, flags: u32) -> isize {
    // 解析路径
    let _guard = SumGuard::new();
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return -(EINVAL as isize);
        }
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
            let _guard = SumGuard::new();
            let user_times = core::slice::from_raw_parts(times, 2);

            // 验证时间结构
            if let Err(e) = user_times[0].validate() {
                return -(e as isize);
            }
            if let Err(e) = user_times[1].validate() {
                return -(e as isize);
            }

            // 处理访问时间
            let atime_opt = if user_times[0].is_omit() {
                None // 不修改
            } else if user_times[0].is_now() {
                Some(TimeSpec::now())
            } else {
                Some(user_times[0])
            };

            // 处理修改时间
            let mtime_opt = if user_times[1].is_omit() {
                None
            } else if user_times[1].is_now() {
                Some(TimeSpec::now())
            } else {
                Some(user_times[1])
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

/// 重命名或移动文件/目录
pub fn renameat2(
    olddirfd: i32,
    oldpath: *const c_char,
    newdirfd: i32,
    newpath: *const c_char,
    flags: u32,
) -> isize {
    use crate::uapi::{
        errno::{EEXIST, ENOTDIR},
        fs::RenameFlags,
    };

    // 解析标志
    let rename_flags = match RenameFlags::from_bits(flags) {
        Some(f) => f,
        None => return -(EINVAL as isize),
    };

    // 检查标志组合的合法性
    if !rename_flags.is_valid() {
        return -(EINVAL as isize);
    }

    // 解析旧路径
    let _guard = SumGuard::new();
    let old_path_str = match get_path_safe(oldpath) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return -(EINVAL as isize);
        }
    };

    // 解析新路径
    let new_path_str = match get_path_safe(newpath) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return -(EINVAL as isize);
        }
    };

    // 分割路径为 (父目录, 文件名)
    let (old_dir_path, old_name) = match split_path(&old_path_str) {
        Ok(p) => p,
        Err(e) => return e.to_errno(),
    };

    let (new_dir_path, new_name) = match split_path(&new_path_str) {
        Ok(p) => p,
        Err(e) => return e.to_errno(),
    };

    // 查找父目录
    let old_parent = match resolve_at_path(olddirfd, &old_dir_path) {
        Ok(Some(d)) => d,
        Ok(None) => return -(ENOENT as isize),
        Err(e) => return e.to_errno(),
    };

    let new_parent = match resolve_at_path(newdirfd, &new_dir_path) {
        Ok(Some(d)) => d,
        Ok(None) => return -(ENOENT as isize),
        Err(e) => return e.to_errno(),
    };

    // 验证父目录是目录
    let old_parent_meta = match old_parent.inode.metadata() {
        Ok(m) => m,
        Err(e) => return e.to_errno(),
    };
    if old_parent_meta.inode_type != InodeType::Directory {
        return -(ENOTDIR as isize);
    }

    let new_parent_meta = match new_parent.inode.metadata() {
        Ok(m) => m,
        Err(e) => return e.to_errno(),
    };
    if new_parent_meta.inode_type != InodeType::Directory {
        return -(ENOTDIR as isize);
    }

    // 查找源文件(验证存在)
    let _old_inode = match old_parent.inode.lookup(&old_name) {
        Ok(inode) => inode,
        Err(e) => return e.to_errno(),
    };

    // 处理不同的重命名标志
    if rename_flags.contains(RenameFlags::EXCHANGE) {
        // ⚠️ 非原子交换实现警告 ⚠️
        //
        // 由于 ext4_rs 缺少事务日志支持，此实现通过三步操作模拟原子交换:
        //   1. old_name -> temp_name
        //   2. new_name -> old_name
        //   3. temp_name -> new_name
        //
        // 安全性限制:
        // - 在步骤 2/3 失败时会尝试回滚，但回滚本身可能失败
        // - 系统崩溃可能导致文件丢失或重复
        // - 不满足 POSIX 的原子性要求
        //
        // 建议:
        // - 仅在非关键场景使用
        // - 操作后调用 sync() 减少崩溃风险

        crate::pr_warn!(
            "[renameat2] EXCHANGE is non-atomic: {} <-> {} (no transaction support)",
            old_name,
            new_name
        );

        // 验证目标文件存在
        let _new_inode = match new_parent.inode.lookup(&new_name) {
            Ok(inode) => inode,
            Err(e) => {
                crate::pr_err!(
                    "[renameat2] EXCHANGE failed: target '{}' does not exist (error: {:?})",
                    new_name,
                    e
                );
                return -(ENOENT as isize); // EXCHANGE 要求目标必须存在
            }
        };

        // 生成临时文件名(使用时间戳或特殊前缀避免冲突)
        let temp_name = alloc::format!(".rename_temp_{}_{}", old_name, new_name);

        crate::pr_debug!(
            "[renameat2] EXCHANGE step 1/3: '{}' -> '{}' (temp)",
            old_name,
            temp_name
        );

        // 步骤1: old_name -> temp_name
        if let Err(e) = old_parent
            .inode
            .rename(&old_name, old_parent.inode.clone(), &temp_name)
        {
            crate::pr_err!(
                "[renameat2] EXCHANGE step 1/3 failed: '{}' -> '{}' (error: {:?})",
                old_name,
                temp_name,
                e
            );
            return e.to_errno();
        }

        crate::pr_debug!(
            "[renameat2] EXCHANGE step 2/3: '{}' -> '{}'",
            new_name,
            old_name
        );

        // 步骤2: new_name -> old_name
        if let Err(e) = new_parent
            .inode
            .rename(&new_name, old_parent.inode.clone(), &old_name)
        {
            crate::pr_err!(
                "[renameat2] EXCHANGE step 2/3 failed: '{}' -> '{}' (error: {:?})",
                new_name,
                old_name,
                e
            );

            // 尝试回滚步骤1
            crate::pr_warn!(
                "[renameat2] Attempting rollback: '{}' -> '{}'",
                temp_name,
                old_name
            );

            match old_parent
                .inode
                .rename(&temp_name, old_parent.inode.clone(), &old_name)
            {
                Ok(_) => {
                    crate::pr_info!("[renameat2] Rollback successful: restored '{}'", old_name);
                }
                Err(rollback_err) => {
                    crate::pr_err!(
                        "[renameat2] CRITICAL: Rollback failed! File '{}' may be lost or duplicated (error: {:?})",
                        old_name,
                        rollback_err
                    );
                    crate::pr_err!(
                        "[renameat2] File system may be in inconsistent state. Temp file '{}' exists.",
                        temp_name
                    );
                }
            }

            return e.to_errno();
        }

        crate::pr_debug!(
            "[renameat2] EXCHANGE step 3/3: '{}' (temp) -> '{}'",
            temp_name,
            new_name
        );

        // 步骤3: temp_name -> new_name
        if let Err(e) = old_parent
            .inode
            .rename(&temp_name, new_parent.inode.clone(), &new_name)
        {
            crate::pr_err!(
                "[renameat2] EXCHANGE step 3/3 failed: '{}' -> '{}' (error: {:?})",
                temp_name,
                new_name,
                e
            );

            // 尝试回滚步骤2和步骤1
            crate::pr_warn!("[renameat2] Attempting full rollback (2 operations)");

            let mut rollback_success = true;

            // 回滚步骤2: old_name -> new_name
            crate::pr_debug!("[renameat2] Rollback 1/2: '{}' -> '{}'", old_name, new_name);
            match old_parent
                .inode
                .rename(&old_name, new_parent.inode.clone(), &new_name)
            {
                Ok(_) => {
                    crate::pr_debug!("[renameat2] Rollback 1/2 successful");
                }
                Err(rollback_err) => {
                    crate::pr_err!(
                        "[renameat2] CRITICAL: Rollback 1/2 failed! '{}' -> '{}' (error: {:?})",
                        old_name,
                        new_name,
                        rollback_err
                    );
                    rollback_success = false;
                }
            }

            // 回滚步骤1: temp_name -> old_name
            crate::pr_debug!(
                "[renameat2] Rollback 2/2: '{}' -> '{}'",
                temp_name,
                old_name
            );
            match old_parent
                .inode
                .rename(&temp_name, old_parent.inode.clone(), &old_name)
            {
                Ok(_) => {
                    crate::pr_debug!("[renameat2] Rollback 2/2 successful");
                }
                Err(rollback_err) => {
                    crate::pr_err!(
                        "[renameat2] CRITICAL: Rollback 2/2 failed! '{}' -> '{}' (error: {:?})",
                        temp_name,
                        old_name,
                        rollback_err
                    );
                    rollback_success = false;
                }
            }

            if rollback_success {
                crate::pr_info!(
                    "[renameat2] Full rollback successful: files restored to original state"
                );
            } else {
                crate::pr_err!("[renameat2] CRITICAL: Partial or complete rollback failure!");
                crate::pr_err!(
                    "[renameat2] File system is in INCONSISTENT STATE. Manual recovery may be required."
                );
                crate::pr_err!(
                    "[renameat2] Affected files: '{}', '{}', temp '{}'",
                    old_name,
                    new_name,
                    temp_name
                );
            }

            return e.to_errno();
        }

        crate::pr_info!(
            "[renameat2] EXCHANGE completed: '{}' <-> '{}'",
            old_name,
            new_name
        );

        // 更新 dentry 缓存
        old_parent.remove_child(&old_name);
        old_parent.remove_child(&temp_name);
        new_parent.remove_child(&new_name);
    } else if rename_flags.contains(RenameFlags::NOREPLACE) {
        // 目标存在时失败
        if new_parent.inode.lookup(&new_name).is_ok() {
            return -(EEXIST as isize);
        }

        // 执行重命名
        if let Err(e) = old_parent
            .inode
            .rename(&old_name, new_parent.inode.clone(), &new_name)
        {
            return e.to_errno();
        }

        // 更新 dentry 缓存
        old_parent.remove_child(&old_name);
    } else if rename_flags.contains(RenameFlags::WHITEOUT) {
        // WHITEOUT 暂不支持(需要 Union FS 支持)
        return FsError::NotSupported.to_errno();
    } else {
        // 普通重命名/移动(允许覆盖目标)
        if let Err(e) = old_parent
            .inode
            .rename(&old_name, new_parent.inode.clone(), &new_name)
        {
            return e.to_errno();
        }

        // 更新 dentry 缓存
        old_parent.remove_child(&old_name);
        new_parent.remove_child(&new_name);
    }

    0
}

/// mount - 挂载文件系统
///
/// # 系统调用号
/// 40 (SYS_MOUNT)
///
/// # 简化实现说明
/// - 只支持 ext4 文件系统（忽略 filesystemtype 参数）
/// - 使用第一个可用的块设备（忽略 source 参数）
/// - 忽略所有 mountflags（但保留以保持 ABI 兼容）
/// - 忽略 data 参数
pub fn mount(
    source: *const c_char,
    target: *const c_char,
    filesystemtype: *const c_char,
    _mountflags: u64,
    _data: *const core::ffi::c_void,
) -> isize {
    use crate::config::EXT4_BLOCK_SIZE;
    use crate::fs::ext4::Ext4FileSystem;
    use crate::fs::sysfs::find_block_device;
    use crate::fs::{init_dev, init_procfs, init_sysfs, mount_tmpfs};
    use crate::vfs::{MOUNT_TABLE, MountFlags as VfsMountFlags};
    use alloc::string::String;

    // 启用用户空间内存访问
    let _guard = SumGuard::new();

    // 解析目标路径
    let target_str = match get_path_safe(target) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return FsError::InvalidArgument.to_errno();
        }
    };

    // 解析 source (可能为空)
    let source_str = if !source.is_null() {
        match get_path_safe(source) {
            Ok(s) => s.to_string(),
            Err(_) => {
                return FsError::InvalidArgument.to_errno();
            }
        }
    } else {
        String::new()
    };

    // 解析 filesystemtype (可能为空)
    let fstype_str = if !filesystemtype.is_null() {
        match get_path_safe(filesystemtype) {
            Ok(s) => s.to_string(),
            Err(_) => {
                return FsError::InvalidArgument.to_errno();
            }
        }
    } else {
        String::new()
    };

    crate::pr_info!(
        "[SYSCALL] mount: source='{}', target='{}', type='{}'",
        source_str,
        target_str,
        fstype_str
    );

    // 特殊挂载点处理
    match target_str.as_str() {
        "/proc" => {
            return match init_procfs() {
                Ok(_) => 0,
                Err(e) => e.to_errno(),
            };
        }
        "/sys" => {
            return match init_sysfs() {
                Ok(_) => 0,
                Err(e) => e.to_errno(),
            };
        }
        "/tmp" => {
            return match mount_tmpfs("/tmp", 0) {
                Ok(_) => 0,
                Err(e) => e.to_errno(),
            };
        }
        "/dev" => {
            // 先挂载 tmpfs 到 /dev
            if let Err(e) = mount_tmpfs("/dev", 0) {
                return e.to_errno();
            }
            // 然后初始化设备节点
            return match init_dev() {
                Ok(_) => 0,
                Err(e) => e.to_errno(),
            };
        }
        _ => {}
    }

    // 通用挂载逻辑 (目前只支持 ext4)
    if fstype_str == "ext4" {
        // 查找块设备
        let dev_info = match find_block_device(&source_str) {
            Some(info) => info,
            None => {
                crate::pr_err!("[SYSCALL] mount: block device '{}' not found", source_str);
                return -(ENOENT as isize);
            }
        };

        let block_device = dev_info.device;

        // 创建 Ext4 文件系统
        let block_size = EXT4_BLOCK_SIZE;
        // 注意：这里我们无法直接知道设备的总块数，
        // 但 Ext4FileSystem::open 通常会读取超级块来获取这些信息，
        // 或者我们可以从 block_device 获取容量。
        // 暂时使用 fs.img 的默认大小计算，或者让 Ext4FileSystem 自己处理。
        // 为了兼容现有 API，我们尝试从设备获取大小。
        let total_blocks = block_device.total_blocks();

        let ext4_fs = match Ext4FileSystem::open(block_device.clone(), block_size, total_blocks, 0)
        {
            Ok(fs) => fs,
            Err(e) => {
                crate::pr_err!("[SYSCALL] mount: failed to open ext4: {:?}", e);
                return e.to_errno();
            }
        };

        // 挂载文件系统
        match MOUNT_TABLE.mount(
            ext4_fs,
            &target_str,
            VfsMountFlags::empty(),
            Some(source_str),
        ) {
            Ok(()) => {
                crate::pr_info!(
                    "[SYSCALL] mount: successfully mounted ext4 at '{}'",
                    target_str
                );
                return 0;
            }
            Err(e) => {
                crate::pr_err!("[SYSCALL] mount: failed: {:?}", e);
                return e.to_errno();
            }
        }
    }

    crate::pr_err!(
        "[SYSCALL] mount: unsupported filesystem type '{}' or target '{}'",
        fstype_str,
        target_str
    );
    -(EINVAL as isize)
}

/// umount2 - 卸载文件系统
///
/// # 系统调用号
/// 39 (SYS_UMOUNT2)
///
/// # 简化实现说明
/// - 忽略 flags 参数（但保留以保持 ABI 兼容）
/// - 不检查文件是否被占用
/// - 直接调用 MOUNT_TABLE.umount()
pub fn umount2(target: *const c_char, _flags: i32) -> isize {
    use crate::vfs::MOUNT_TABLE;

    // 启用用户空间内存访问
    let _guard = SumGuard::new();

    // 解析目标路径
    let target_str = match get_path_safe(target) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return FsError::InvalidArgument.to_errno();
        }
    };

    crate::pr_info!("[SYSCALL] umount2: unmounting '{}'", target_str);

    // 卸载文件系统

    // 注意：MOUNT_TABLE.umount() 会自动调用 fs.sync()
    match MOUNT_TABLE.umount(&target_str) {
        Ok(()) => {
            crate::pr_info!("[SYSCALL] umount2: successfully unmounted '{}'", target_str);
            0
        }
        Err(e) => {
            crate::pr_err!("[SYSCALL] umount2: failed: {:?}", e);
            e.to_errno()
        }
    }
}

/// 同步所有文件系统
///
/// # 实现说明
/// 由于 Comix 使用写直达架构,数据已在块设备中,
/// 此调用只需刷新硬件写缓存。
pub fn sync() -> isize {
    use crate::kernel::syscall::util::flush_all_block_devices;

    // sync 总是成功(即使 flush 失败也不返回错误)
    let _ = flush_all_block_devices();
    0
}

/// syncfs - 同步指定文件系统
pub fn syncfs(fd: usize) -> isize {
    use crate::kernel::syscall::util::flush_block_device_by_fd;

    match flush_block_device_by_fd(fd) {
        Ok(()) => 0,
        Err(errno) => errno,
    }
}

/// fsync - 同步文件数据和元数据
///
/// # 实现说明
/// 在写直达架构下,等同于 syncfs
pub fn fsync(fd: usize) -> isize {
    syncfs(fd)
}

/// fdatasync - 同步文件数据(元数据可选)
///
/// # 实现说明
/// 在写直达架构下,完全等同于 fsync
pub fn fdatasync(fd: usize) -> isize {
    fsync(fd)
}

/// fchownat - 修改文件所有者和组
///
/// # 参数
/// * `dirfd` - 目录文件描述符（AT_FDCWD 表示当前目录）
/// * `pathname` - 文件路径
/// * `owner` - 新的用户 ID（u32::MAX 表示不改变）
/// * `group` - 新的组 ID（u32::MAX 表示不改变）
/// * `flags` - 标志位（AT_SYMLINK_NOFOLLOW、AT_EMPTY_PATH 等）
///
/// # 返回值
/// * 0 - 成功
/// * -errno - 失败
///
/// # 在单 root 用户系统中的行为
/// 所有调用都会成功并更新 inode 的 uid/gid 字段，不进行权限检查
pub fn fchownat(dirfd: i32, pathname: *const c_char, owner: u32, group: u32, flags: u32) -> isize {
    use crate::uapi::fs::AtFlags;

    // 解析路径字符串
    let _guard = SumGuard::new();
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return -(EINVAL as isize);
        }
    };

    // 解析标志位
    let at_flags = AtFlags::from_bits_truncate(flags);
    let follow_symlink = !at_flags.contains(AtFlags::SYMLINK_NOFOLLOW);
    let empty_path = at_flags.contains(AtFlags::EMPTY_PATH);

    // 处理 AT_EMPTY_PATH 情况
    if empty_path && path_str.is_empty() {
        // pathname 为空，操作 dirfd 本身
        if dirfd == AT_FDCWD {
            // 不能对当前目录使用 AT_EMPTY_PATH
            return -(EINVAL as isize);
        }

        let task = current_task();
        let file = match task.lock().fd_table.get(dirfd as usize) {
            Ok(f) => f,
            Err(e) => return e.to_errno(),
        };

        let dentry = match file.dentry() {
            Ok(d) => d,
            Err(e) => return e.to_errno(),
        };

        return match dentry.inode.chown(owner, group) {
            Ok(()) => 0,
            Err(e) => e.to_errno(),
        };
    }

    // 解析路径，获取 dentry（使用辅助函数）
    let dentry = match resolve_at_path_with_flags(dirfd, &path_str, follow_symlink) {
        Ok(d) => d,
        Err(e) => return e.to_errno(),
    };

    // 调用 inode 的 chown 方法
    match dentry.inode.chown(owner, group) {
        Ok(()) => 0,
        Err(e) => e.to_errno(),
    }
}

/// fchmodat - 修改文件权限模式
///
/// # 参数
/// * `dirfd` - 目录文件描述符（AT_FDCWD 表示当前目录）
/// * `pathname` - 文件路径
/// * `mode` - 新的权限模式（12 位权限位）
/// * `flags` - 标志位（AT_SYMLINK_NOFOLLOW 等）
///
/// # 返回值
/// * 0 - 成功
/// * -errno - 失败
///
/// # 在单 root 用户系统中的行为
/// 所有调用都会成功并更新 inode 的 mode 字段，不进行权限检查
pub fn fchmodat(dirfd: i32, pathname: *const c_char, mode: u32, flags: u32) -> isize {
    use crate::uapi::fs::AtFlags;

    // 解析路径字符串
    let _guard = SumGuard::new();
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return -(EINVAL as isize);
        }
    };

    // 解析标志位
    let at_flags = AtFlags::from_bits_truncate(flags);
    let follow_symlink = !at_flags.contains(AtFlags::SYMLINK_NOFOLLOW);

    // 验证 mode 参数（只保留权限位，去除文件类型位）
    let mode = mode & 0o7777; // 保留 12 位权限位（包括 setuid/setgid/sticky）
    let file_mode = match FileMode::from_bits(mode) {
        Some(m) => m,
        None => return -(EINVAL as isize),
    };

    // 解析路径，获取 dentry（使用辅助函数）
    let dentry = match resolve_at_path_with_flags(dirfd, &path_str, follow_symlink) {
        Ok(d) => d,
        Err(e) => return e.to_errno(),
    };

    // 调用 inode 的 chmod 方法
    match dentry.inode.chmod(file_mode) {
        Ok(()) => 0,
        Err(e) => e.to_errno(),
    }
}

/// mknodat 系统调用
///
/// # 参数
/// - `dirfd`: 目录文件描述符（-100 表示当前工作目录）
/// - `pathname`: 路径名
/// - `mode`: 文件模式（包含类型和权限）
/// - `dev`: 设备号
///
/// # 返回
/// * 0: 成功
/// * -errno - 失败
pub fn mknodat(dirfd: i32, pathname: *const c_char, mode: u32, dev: u64) -> isize {
    // 安全地读取路径字符串
    let _guard = SumGuard::new();
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return FsError::InvalidArgument.to_errno();
        }
    };

    // 分割路径为目录和文件名
    let (dir_path, filename) = match split_path(&path_str) {
        Ok(p) => p,
        Err(e) => return e.to_errno(),
    };

    // 解析父目录路径
    let parent_dentry = match resolve_at_path(dirfd, &dir_path) {
        Ok(Some(d)) => d,
        Ok(None) => return FsError::NotFound.to_errno(),
        Err(e) => return e.to_errno(),
    };

    // 构造文件模式
    let file_mode = FileMode::from_bits_truncate(mode);

    // 调用 inode.mknod()
    match parent_dentry.inode.mknod(&filename, file_mode, dev) {
        Ok(child_inode) => {
            // 创建 dentry 并加入缓存
            let child_dentry = Dentry::new(filename.clone(), child_inode);
            parent_dentry.add_child(child_dentry.clone());
            DENTRY_CACHE.insert(&child_dentry);
            0
        }
        Err(e) => e.to_errno(),
    }
}

/// symlinkat - 创建符号链接
///
/// # 参数
/// * `target` - 符号链接的目标路径(可以是相对或绝对路径)
/// * `newdirfd` - 新符号链接所在目录的文件描述符
/// * `linkpath` - 新符号链接的路径
///
/// # 返回值
/// * 0 - 成功
/// * -errno - 失败
///
/// # 注意
/// target 参数不会被检查,即使目标不存在也能创建符号链接
pub fn symlinkat(target: *const c_char, newdirfd: i32, linkpath: *const c_char) -> isize {
    // 解析 target 路径
    let _guard = SumGuard::new();
    let target_str = match get_path_safe(target) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return FsError::InvalidArgument.to_errno();
        }
    };

    // 解析 linkpath 路径
    let link_str = match get_path_safe(linkpath) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return FsError::InvalidArgument.to_errno();
        }
    };

    // 分割路径为目录和文件名
    let (dir_path, link_name) = match split_path(&link_str) {
        Ok(p) => p,
        Err(e) => return e.to_errno(),
    };

    // 查找父目录
    let parent_dentry = match resolve_at_path(newdirfd, &dir_path) {
        Ok(Some(d)) => d,
        Ok(None) => return FsError::NotFound.to_errno(),
        Err(e) => return e.to_errno(),
    };

    // 创建符号链接
    match parent_dentry.inode.symlink(&link_name, &target_str) {
        Ok(symlink_inode) => {
            // 创建 dentry 并加入缓存
            let symlink_dentry = Dentry::new(link_name.clone(), symlink_inode);
            parent_dentry.add_child(symlink_dentry.clone());
            DENTRY_CACHE.insert(&symlink_dentry);
            0
        }
        Err(e) => e.to_errno(),
    }
}
