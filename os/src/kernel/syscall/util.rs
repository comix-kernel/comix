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
    vfs::{
        DENTRY_CACHE, Dentry, FileMode, FsError, InodeType, get_root_dentry, split_path,
        vfs_lookup_from,
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
/// - 成功时返回包含参数字符串的 Vec<String>
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
