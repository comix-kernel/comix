use super::*;

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
    let path_str = match get_path_safe(pathname as usize) {
        Ok(s) => s,
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
    let path_str = match get_path_safe(pathname as usize) {
        Ok(s) => s,
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
    let path_str = match get_path_safe(pathname as usize) {
        Ok(s) => s,
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
    let target_str = match get_path_safe(target as usize) {
        Ok(s) => s,
        Err(_) => {
            return FsError::InvalidArgument.to_errno();
        }
    };

    // 解析 linkpath 路径
    let link_str = match get_path_safe(linkpath as usize) {
        Ok(s) => s,
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
