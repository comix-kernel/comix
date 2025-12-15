//! 系统调用辅助函数

use core::ffi::{CStr, c_char};

use alloc::{
    format,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

use crate::{
    kernel::current_task,
    uapi::{errno::EINVAL, log::SyslogAction},
    vfs::{
        DENTRY_CACHE, Dentry, File, FileMode, FsError, InodeType, OpenFlags, get_root_dentry,
        impls::{BlockDeviceFile, CharDeviceFile, RegFile},
        split_path, vfs_lookup_from,
    },
};

/// 从用户空间获取路径字符串
/// # 参数
/// - `path`: 指向用户空间路径字符串的指针
/// # 返回值
/// - 成功时返回路径字符串的引用
/// - 失败时返回错误字符串
pub fn get_path_safe(path: *const c_char) -> Result<&'static str, &'static str> {
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

/// 从用户空间获取参数字符串数组
///# 参数
/// - `ptr_array`: 指向用户空间字符串指针数组的指针
/// - `name`: 参数名称，用于错误报告
/// # 返回值
/// - 成功时返回包含参数字符串的 `Vec<String>`
/// - 失败时返回错误字符串
pub fn get_args_safe(
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
pub fn resolve_at_path(dirfd: i32, path: &str) -> Result<Option<Arc<Dentry>>, FsError> {
    let base_dentry = if path.starts_with('/') {
        get_root_dentry()?
    } else if dirfd == super::fs::AT_FDCWD {
        current_task()
            .lock()
            .fs
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

/// 在指定目录下创建一个新文件
///// # 参数
/// - `dirfd`: 目录文件描述符，或 AT_FDCWD
/// - `path`: 要创建的文件路径（相对于 dirfd）
/// - `mode`: 文件权限模式
/// 返回新创建的文件的 Dentry
pub fn create_file_at(dirfd: i32, path: &str, mode: u32) -> Result<Arc<Dentry>, FsError> {
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

/// 验证 syslog 系统调用参数
///
/// 根据操作类型检查参数的有效性。
///
/// # 验证规则
/// - **Read/ReadAll/ReadClear**: bufp != NULL, len >= 0
/// - **ConsoleLevel**: len 必须在 1-8 范围内
/// - **其他操作**: 忽略 bufp 和 len
///
/// # 返回值
/// * `Ok(())` - 参数有效
/// * `Err(EINVAL)` - 参数无效
pub fn validate_syslog_args(action: SyslogAction, bufp: *mut u8, len: i32) -> Result<(), i32> {
    match action {
        SyslogAction::Read | SyslogAction::ReadAll | SyslogAction::ReadClear => {
            // 这些操作需要有效的缓冲区
            if bufp.is_null() {
                return Err(EINVAL);
            }
            if len < 0 {
                return Err(EINVAL);
            }
            // len == 0 是合法的，只是不会读取任何数据
        }

        SyslogAction::ConsoleLevel => {
            // Linux 要求 console_loglevel 在 1-8 范围内
            // 参考：kernel/printk/printk.c
            if len < 1 || len > 8 {
                return Err(EINVAL);
            }
        }

        _ => {
            // 其他操作不需要验证 bufp 和 len
        }
    }

    Ok(())
}

/// 检查 syslog 操作权限
///
/// # 权限规则（完全遵循 Linux）
/// 1. **特殊情况：ReadAll 和 SizeBuffer**
///    - 如果 `dmesg_restrict == 0`：允许所有用户访问
///    - 如果 `dmesg_restrict != 0`：需要特权
/// 2. **其他操作**：
///    - 需要以下任一权限：
///      - `euid == 0` (root 用户)
///      - `CAP_SYSLOG` (推荐)
///      - `CAP_SYS_ADMIN` (向后兼容)
///
/// # 返回值
/// * `Ok(())` - 有权限
/// * `Err(EPERM)` - 权限不足
pub fn check_syslog_permission(action: SyslogAction) -> Result<(), i32> {
    // TODO: 等待完成用户管理和能力模型后再实现完整的权限检查
    //
    // 完整实现应该包括：
    // 1. 检查 dmesg_restrict sysctl (ReadAll 和 SizeBuffer 特殊处理)
    // 2. 检查 euid == 0 (root 用户)
    // 3. 检查 CAP_SYSLOG 能力
    // 4. 检查 CAP_SYS_ADMIN 能力 (向后兼容)
    //
    // 临时实现：允许所有操作（开发阶段）

    // 特殊处理：ReadAll 和 SizeBuffer 可能允许非特权访问
    if matches!(action, SyslogAction::ReadAll | SyslogAction::SizeBuffer) {
        // 检查 dmesg_restrict sysctl
        let dmesg_restrict = get_dmesg_restrict();
        if dmesg_restrict == 0 {
            return Ok(()); // 允许非特权访问
        }
    }

    // TODO: 完整的权限检查实现
    // 临时方案：暂时允许所有操作
    Ok(())

    /* 完整实现参考（等待用户管理模块）：

    // 获取当前任务
    let task = current_task();
    let task_locked = task.lock();

    // 检查是否为 root
    if task_locked.euid == 0 {
        return Ok(());
    }

    // 检查 CAP_SYSLOG (Linux 2.6.38+)
    if task_locked.capabilities.has_effective(Capability::CAP_SYSLOG) {
        return Ok(());
    }

    // 检查 CAP_SYS_ADMIN (向后兼容)
    if task_locked.capabilities.has_effective(Capability::CAP_SYS_ADMIN) {
        return Ok(());
    }

    // 权限不足
    Err(EPERM)
    */
}

/// 获取 dmesg_restrict sysctl 值
///
/// # 返回值
/// * `0` - 允许所有用户读取内核日志
/// * `1` - 只允许特权用户读取
///
/// # TODO
/// 目前硬编码为 0，需要实现真实的 sysctl 支持。
#[inline]
fn get_dmesg_restrict() -> u32 {
    // TODO: 从 sysctl 系统读取真实值
    // 参考路径: /proc/sys/kernel/dmesg_restrict
    0
}

/// 获取第一个可用的块设备驱动
///
/// 简化版实现：直接返回第一个块设备
///
/// # 返回值
/// * `Ok(Arc<dyn BlockDriver>)` - 成功获取块设备
/// * `Err(errno)` - 没有可用的块设备
pub fn get_first_block_device() -> Result<Arc<dyn crate::device::block::BlockDriver>, i32> {
    use crate::device::BLK_DRIVERS;
    use crate::uapi::errno::ENODEV;

    let drivers = BLK_DRIVERS.read();

    if drivers.is_empty() {
        return Err(-ENODEV);
    }

    Ok(drivers[0].clone())
}

/// 刷新所有块设备
///
/// # 返回值
/// 如果所有设备成功刷新返回 Ok(()),否则返回第一个错误
pub fn flush_all_block_devices() -> Result<(), isize> {
    use crate::{device::BLK_DRIVERS, uapi::errno::EIO};

    let drivers = BLK_DRIVERS.read();

    if drivers.is_empty() {
        // 没有块设备也算成功(无事可做)
        return Ok(());
    }

    for driver in drivers.iter() {
        if !driver.flush() {
            // VirtIO flush 失败
            return Err(-EIO as isize);
        }
    }

    Ok(())
}

/// 从文件描述符获取对应的块设备并刷新
///
/// 通过 fd -> dentry -> 路径 -> 挂载点 -> 文件系统 -> 同步
///
/// # 参数
/// - `fd`: 文件描述符
///
/// # 返回值
/// - Ok(()): 刷新成功
/// - Err(-EBADF): 无效的文件描述符
/// - Err(-EINVAL): fd 不支持同步(如 pipe、socket)
/// - Err(-EIO): 块设备刷新失败
pub fn flush_block_device_by_fd(fd: usize) -> Result<(), isize> {
    use crate::{uapi::errno::EIO, vfs::MOUNT_TABLE};

    // 1. 获取文件对象
    let task = current_task();
    let file = task.lock().fd_table.get(fd).map_err(|e| e.to_errno())?;

    // 2. 获取 dentry (如果不支持则说明是管道等特殊文件)
    let dentry = file.dentry().map_err(|e| e.to_errno())?;

    // 3. 获取文件的完整路径
    let path = dentry.full_path();

    // 4. 通过路径查找对应的挂载点
    let mount_point = MOUNT_TABLE
        .find_mount(&path)
        .ok_or_else(|| FsError::NotSupported.to_errno())?;

    // 5. 调用文件系统的 sync 方法
    mount_point.fs.sync().map_err(|_| -EIO as isize)?;

    Ok(())
}

/// 根据 dirfd 和路径解析 dentry，支持符号链接控制
///
/// 这是对 `resolve_at_path` 的扩展，支持控制是否跟随最后一个符号链接。
///
/// # 参数
/// * `dirfd` - 目录文件描述符（AT_FDCWD 表示当前目录）
/// * `path` - 文件路径
/// * `follow_symlink` - 是否跟随最后一个符号链接
///
/// # 返回值
/// * `Ok(Arc<Dentry>)` - 成功找到文件
/// * `Err(FsError)` - 查找失败
///
/// # 示例
/// ```rust
/// // 跟随符号链接（默认行为）
/// let dentry = resolve_at_path_with_flags(AT_FDCWD, "/path/to/file", true)?;
///
/// // 不跟随符号链接（用于 lstat, lchown 等）
/// let dentry = resolve_at_path_with_flags(AT_FDCWD, "/path/to/symlink", false)?;
/// ```
pub fn resolve_at_path_with_flags(
    dirfd: i32,
    path: &str,
    follow_symlink: bool,
) -> Result<Arc<Dentry>, FsError> {
    use crate::vfs::vfs_lookup_no_follow;

    if follow_symlink {
        // 跟随符号链接，使用标准的 resolve_at_path
        resolve_at_path(dirfd, path)?.ok_or(FsError::NotFound)
    } else {
        // 不跟随符号链接
        if path.starts_with('/') {
            // 绝对路径
            vfs_lookup_no_follow(path)
        } else {
            // 相对路径，需要从 dirfd 开始
            let base_dentry = if dirfd == super::fs::AT_FDCWD {
                // 使用当前工作目录
                current_task()
                    .lock()
                    .fs
                    .lock()
                    .cwd
                    .clone()
                    .ok_or(FsError::NotFound)?
            } else {
                // 使用 dirfd 指向的目录
                let task = current_task();
                let file = task.lock().fd_table.get(dirfd as usize)?;
                file.dentry()?
            };

            // 构建完整路径
            let full_path = if base_dentry.full_path() == "/" {
                format!("/{}", path)
            } else {
                format!("{}/{}", base_dentry.full_path(), path)
            };

            vfs_lookup_no_follow(&full_path)
        }
    }
}

/// 根据 inode 类型创建对应的 File 实例
pub fn create_file_from_dentry(
    dentry: Arc<Dentry>,
    flags: OpenFlags,
) -> Result<Arc<dyn File>, FsError> {
    let inode_type = dentry.inode.metadata()?.inode_type;

    let file: Arc<dyn File> = match inode_type {
        InodeType::File | InodeType::Directory | InodeType::Symlink => {
            // 普通文件、目录、符号链接
            Arc::new(RegFile::new(dentry, flags))
        }
        InodeType::CharDevice => {
            // 字符设备
            Arc::new(CharDeviceFile::new(dentry, flags)?)
        }
        InodeType::BlockDevice => {
            // 块设备
            Arc::new(BlockDeviceFile::new(dentry, flags)?)
        }
        InodeType::Socket | InodeType::Fifo => {
            // FIFO（命名管道） 和 Unix 域套接字 暂不支持
            return Err(FsError::NotSupported);
        }
    };

    Ok(file)
}
