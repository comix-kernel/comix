//! 文件系统相关的系统调用实现

use core::ffi::c_char;

use alloc::{string::ToString, sync::Arc};
use riscv::register::sstatus;

use crate::{
    kernel::{
        current_cpu, current_task,
        syscall::util::{create_file_at, get_path_safe, resolve_at_path},
    },
    uapi::{
        errno::{EACCES, EINVAL, ENOENT},
        fs::{AtFlags, F_OK, FileSystemType, LinuxStatFs, R_OK, W_OK, X_OK},
        time::timespec,
    },
    vfs::{
        Dentry, DiskFile, FileMode, FsError, InodeType, OpenFlags, SeekWhence, Stat, split_path,
        vfs_lookup,
    },
};

pub const AT_FDCWD: i32 = -100;
pub const AT_SYMLINK_NOFOLLOW: u32 = 0x100;
pub const AT_REMOVEDIR: u32 = 0x200;
pub const O_CLOEXEC: u32 = 0o2000000;

pub fn close(fd: usize) -> isize {
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    match task.lock().fd_table.close(fd) {
        Ok(()) => 0,
        Err(e) => e.to_errno(),
    }
}

pub fn lseek(fd: usize, offset: isize, whence: usize) -> isize {
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
pub fn openat(dirfd: i32, pathname: *const c_char, flags: u32, mode: u32) -> isize {
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

pub fn mkdirat(dirfd: i32, pathname: *const c_char, mode: u32) -> isize {
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

pub fn unlinkat(dirfd: i32, pathname: *const c_char, flags: u32) -> isize {
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

pub fn chdir(path: *const c_char) -> isize {
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
        sstatus::set_sum();
        core::ptr::copy_nonoverlapping(path_bytes.as_ptr(), buf, path_bytes.len());
        *buf.add(path_bytes.len()) = 0; // null terminator
        sstatus::clear_sum();
    }

    buf as isize
}

pub fn fstat(fd: usize, statbuf: *mut Stat) -> isize {
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

pub fn getdents64(fd: usize, dirp: *mut u8, count: usize) -> isize {
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

pub fn statfs(path: *const c_char, buf: *mut LinuxStatFs) -> isize {
    // 参数校验
    if buf.is_null() {
        return -(EINVAL as isize);
    }

    // 解析路径字符串
    unsafe { sstatus::set_sum() };
    let path_str = match get_path_safe(path) {
        Ok(s) => s.to_string(),
        Err(_) => {
            unsafe { sstatus::clear_sum() };
            return -(EINVAL as isize);
        }
    };
    unsafe { sstatus::clear_sum() };

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
    unsafe {
        sstatus::set_sum();
        core::ptr::write(buf, statfs_buf);
        sstatus::clear_sum();
    }

    0
}

pub fn faccessat(dirfd: i32, pathname: *const c_char, mode: i32, flags: u32) -> isize {
    // 解析路径字符串
    unsafe { sstatus::set_sum() };
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
        Err(_) => {
            unsafe { sstatus::clear_sum() };
            return -(EINVAL as isize);
        }
    };
    unsafe { sstatus::clear_sum() };

    // 解析标志
    let at_flags = match AtFlags::from_bits(flags) {
        Some(f) => f,
        None => return -(EINVAL as isize),
    };

    // 查找文件
    let dentry = if at_flags.contains(AtFlags::SYMLINK_NOFOLLOW) {
        // 不跟随符号链接：需要特殊处理最后一级路径
        let (dir_path, filename) = match split_path(&path_str) {
            Ok(p) => p,
            Err(e) => return e.to_errno(),
        };

        // 解析到父目录
        let parent_dentry = match resolve_at_path(dirfd, &dir_path) {
            Ok(Some(d)) => d,
            Ok(None) => return -(ENOENT as isize),
            Err(e) => return e.to_errno(),
        };

        // 在父目录中查找文件名（不跟随符号链接）
        if let Some(child) = parent_dentry.lookup_child(&filename) {
            child
        } else {
            let child_inode = match parent_dentry.inode.lookup(&filename) {
                Ok(i) => i,
                Err(e) => return e.to_errno(),
            };
            let child_dentry = Dentry::new(filename.clone(), child_inode);
            parent_dentry.add_child(child_dentry.clone());
            child_dentry
        }
    } else {
        // 跟随符号链接（默认行为）
        match resolve_at_path(dirfd, &path_str) {
            Ok(Some(d)) => d,
            Ok(None) => return -(ENOENT as isize),
            Err(e) => return e.to_errno(),
        }
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
    unsafe { sstatus::set_sum() };
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
        Err(_) => {
            unsafe { sstatus::clear_sum() };
            return -(EINVAL as isize);
        }
    };
    unsafe { sstatus::clear_sum() };

    // 查找符号链接（不跟随最后一级的符号链接）
    let dentry = {
        let (dir_path, filename) = match split_path(&path_str) {
            Ok(p) => p,
            Err(e) => return e.to_errno(),
        };

        // 解析到父目录（路径中间的符号链接会被跟随）
        let parent_dentry = match resolve_at_path(dirfd, &dir_path) {
            Ok(Some(d)) => d,
            Ok(None) => return -(ENOENT as isize),
            Err(e) => return e.to_errno(),
        };

        // 在父目录中查找文件名（不跟随符号链接）
        if let Some(child) = parent_dentry.lookup_child(&filename) {
            child
        } else {
            let child_inode = match parent_dentry.inode.lookup(&filename) {
                Ok(i) => i,
                Err(e) => return e.to_errno(),
            };
            let child_dentry = Dentry::new(filename.clone(), child_inode);
            parent_dentry.add_child(child_dentry.clone());
            child_dentry
        }
    };

    // 验证是符号链接
    let meta = match dentry.inode.metadata() {
        Ok(m) => m,
        Err(e) => return e.to_errno(),
    };

    if meta.inode_type != InodeType::Symlink {
        return -(EINVAL as isize);
    }

    // 读取符号链接目标（使用 read_at）
    let read_size = core::cmp::min(meta.size, bufsiz);
    let mut temp_buf = alloc::vec![0u8; read_size];

    let bytes_read = match dentry.inode.read_at(0, &mut temp_buf) {
        Ok(n) => n,
        Err(e) => return e.to_errno(),
    };

    // 复制到用户空间（注意：readlink 不添加 null 终止符）
    unsafe {
        sstatus::set_sum();
        core::ptr::copy_nonoverlapping(temp_buf.as_ptr(), buf, bytes_read);
        sstatus::clear_sum();
    }

    bytes_read as isize
}

pub fn newfstatat(dirfd: i32, pathname: *const c_char, statbuf: *mut Stat, flags: u32) -> isize {
    // 参数校验
    if statbuf.is_null() {
        return -(EINVAL as isize);
    }

    // 解析路径
    unsafe { sstatus::set_sum() };
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
        Err(_) => {
            unsafe { sstatus::clear_sum() };
            return -(EINVAL as isize);
        }
    };
    unsafe { sstatus::clear_sum() };

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
    let dentry = if at_flags.contains(AtFlags::SYMLINK_NOFOLLOW) {
        // 不跟随符号链接：需要特殊处理最后一级路径
        let (dir_path, filename) = match split_path(&path_str) {
            Ok(p) => p,
            Err(e) => return e.to_errno(),
        };

        // 解析到父目录
        let parent_dentry = match resolve_at_path(dirfd, &dir_path) {
            Ok(Some(d)) => d,
            Ok(None) => return -(ENOENT as isize),
            Err(e) => return e.to_errno(),
        };

        // 在父目录中查找文件名（不跟随符号链接）
        // 先检查 dentry 缓存
        if let Some(child) = parent_dentry.lookup_child(&filename) {
            child
        } else {
            // 缓存未命中，通过 inode 查找
            let child_inode = match parent_dentry.inode.lookup(&filename) {
                Ok(i) => i,
                Err(e) => return e.to_errno(),
            };

            // 创建新的 dentry
            let child_dentry = Dentry::new(filename.clone(), child_inode);
            parent_dentry.add_child(child_dentry.clone());
            child_dentry
        }
    } else {
        // 跟随符号链接（默认行为）
        match resolve_at_path(dirfd, &path_str) {
            Ok(Some(d)) => d,
            Ok(None) => return -(ENOENT as isize),
            Err(e) => return e.to_errno(),
        }
    };

    // 获取文件元数据
    let metadata = match dentry.inode.metadata() {
        Ok(m) => m,
        Err(e) => return e.to_errno(),
    };

    // 转换为 Stat 结构
    let stat = Stat::from_metadata(&metadata);

    // 写回用户空间
    unsafe {
        sstatus::set_sum();
        core::ptr::write(statbuf, stat);
        sstatus::clear_sum();
    }

    0
}

pub fn utimensat(dirfd: i32, pathname: *const c_char, times: *const timespec, flags: u32) -> isize {
    // 解析路径
    unsafe { sstatus::set_sum() };
    let path_str = match get_path_safe(pathname) {
        Ok(s) => s.to_string(),
        Err(_) => {
            unsafe { sstatus::clear_sum() };
            return -(EINVAL as isize);
        }
    };
    unsafe { sstatus::clear_sum() };

    // 解析标志
    let at_flags = match AtFlags::from_bits(flags) {
        Some(f) => f,
        None => return -(EINVAL as isize),
    };

    // 查找文件
    let dentry = if at_flags.contains(AtFlags::SYMLINK_NOFOLLOW) {
        // 不跟随符号链接
        let (dir_path, filename) = match split_path(&path_str) {
            Ok(p) => p,
            Err(e) => return e.to_errno(),
        };

        let parent_dentry = match resolve_at_path(dirfd, &dir_path) {
            Ok(Some(d)) => d,
            Ok(None) => return -(ENOENT as isize),
            Err(e) => return e.to_errno(),
        };

        if let Some(child) = parent_dentry.lookup_child(&filename) {
            child
        } else {
            let child_inode = match parent_dentry.inode.lookup(&filename) {
                Ok(i) => i,
                Err(e) => return e.to_errno(),
            };
            let child_dentry = Dentry::new(filename.clone(), child_inode);
            parent_dentry.add_child(child_dentry.clone());
            child_dentry
        }
    } else {
        // 跟随符号链接
        match resolve_at_path(dirfd, &path_str) {
            Ok(Some(d)) => d,
            Ok(None) => return -(ENOENT as isize),
            Err(e) => return e.to_errno(),
        }
    };

    // 解析时间参数
    let (atime_opt, mtime_opt) = if times.is_null() {
        // NULL 表示将两个时间都设置为当前时间
        let now = timespec::now();
        (Some(now), Some(now))
    } else {
        unsafe {
            sstatus::set_sum();
            let user_times = core::slice::from_raw_parts(times, 2);

            // 验证时间结构
            if let Err(e) = user_times[0].validate() {
                sstatus::clear_sum();
                return -(e as isize);
            }
            if let Err(e) = user_times[1].validate() {
                sstatus::clear_sum();
                return -(e as isize);
            }

            // 处理访问时间
            let atime_opt = if user_times[0].is_omit() {
                None // 不修改
            } else if user_times[0].is_now() {
                Some(timespec::now())
            } else {
                Some(user_times[0])
            };

            // 处理修改时间
            let mtime_opt = if user_times[1].is_omit() {
                None
            } else if user_times[1].is_now() {
                Some(timespec::now())
            } else {
                Some(user_times[1])
            };

            sstatus::clear_sum();
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
    unsafe { sstatus::set_sum() };
    let old_path_str = match get_path_safe(oldpath) {
        Ok(s) => s.to_string(),
        Err(_) => {
            unsafe { sstatus::clear_sum() };
            return -(EINVAL as isize);
        }
    };

    // 解析新路径
    let new_path_str = match get_path_safe(newpath) {
        Ok(s) => s.to_string(),
        Err(_) => {
            unsafe { sstatus::clear_sum() };
            return -(EINVAL as isize);
        }
    };
    unsafe { sstatus::clear_sum() };

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
