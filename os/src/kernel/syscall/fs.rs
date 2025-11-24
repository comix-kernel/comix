//! 文件系统相关的系统调用实现

use core::ffi::c_char;

use alloc::{string::ToString, sync::Arc};
use riscv::register::sstatus;

use crate::{
    kernel::{
        current_cpu, current_task,
        syscall::util::{create_file_at, get_path_safe, resolve_at_path},
    },
    vfs::{
        DiskFile, FileMode, FsError, InodeType, OpenFlags, SeekWhence, Stat, split_path, vfs_lookup,
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
    current_task().lock().cwd = Some(dentry);
    0
}

pub fn getcwd(buf: *mut u8, size: usize) -> isize {
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
