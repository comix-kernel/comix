//! 路径解析引擎
//!
//! 该模块实现了 VFS 的路径解析功能，负责将路径字符串转换为 Dentry，支持绝对路径、
//! 相对路径和符号链接解析。
//!
//! # 核心组件
//!
//! - [`PathComponent`] - 路径组件枚举（Root、Parent、Current、Normal）
//! - [`parse_path`] - 路径字符串解析器
//! - [`normalize_path`] - 路径规范化函数
//! - [`split_path`] - 路径分割函数
//! - [`vfs_lookup`] - 主路径查找函数
//! - [`vfs_lookup_no_follow`] - 不跟随符号链接的查找
//!
//! # 路径解析流程
//!
//! 完整的路径查找需要经过多个步骤：
//!
//! ```text
//! 用户输入: "/home/../etc/./passwd"
//!     ↓
//! parse_path() → [Root, Normal("home"), Parent, Normal("etc"), Current, Normal("passwd")]
//!     ↓
//! normalize_path() → "/etc/passwd"
//!     ↓
//! 检查缓存 (DENTRY_CACHE)
//!     ├─ 命中 → 返回缓存的 Dentry
//!     └─ 未命中 ↓
//! vfs_lookup() - 逐级查找
//!     ├─ "/" → 根 Dentry
//!     ├─ "etc" → 查找子项，检查挂载点
//!     └─ "passwd" → 最终 Dentry
//! ```
//!
//! # 路径组件
//!
//! ## PathComponent 枚举
//!
//! ```rust
//! pub enum PathComponent {
//!     Root,           // "/"
//!     Current,        // "."
//!     Parent,         // ".."
//!     Normal(String), // 普通文件名
//! }
//! ```
//!
//! ### 解析示例
//!
//! ```text
//! "/home/user/file.txt"  → [Root, Normal("home"), Normal("user"), Normal("file.txt")]
//! "../file.txt"          → [Parent, Normal("file.txt")]
//! "./file.txt"           → [Current, Normal("file.txt")]
//! "//foo///bar//"        → [Root, Normal("foo"), Normal("bar")]
//! ```
//!
//! # 路径规范化
//!
//! `normalize_path()` 处理 `.` 和 `..`，移除冗余斜杠：
//!
//! ```rust
//! normalize_path("/home/../etc/./passwd")  // → "/etc/passwd"
//! normalize_path("./file.txt")             // → "file.txt"
//! normalize_path("///foo//bar//")          // → "/foo/bar"
//! normalize_path("/../../../etc")          // → "/etc" (不能越过根目录)
//! ```
//!
//! # 符号链接处理
//!
//! ## 跟随符号链接
//!
//! `vfs_lookup()` 默认跟随符号链接，最多 8 层：
//!
//! ```text
//! /link1 → /link2 → /link3 → /real_file
//!   ↓        ↓        ↓         ↓
//! 解析    解析     解析      返回
//! ```
//!
//! ## 不跟随符号链接
//!
//! `vfs_lookup_no_follow()` 返回符号链接本身：
//!
//! ```rust
//! // 对于符号链接 /link → /target
//! vfs_lookup("/link")?;           // 返回 /target 的 Dentry
//! vfs_lookup_no_follow("/link")?; // 返回 /link 的 Dentry
//! ```
//!
//! # 挂载点处理
//!
//! 查找过程中自动处理挂载点：
//!
//! ```text
//! /mnt 挂载了 tmpfs
//!
//! 查找 "/mnt/file"：
//!   1. 找到 /mnt 的 Dentry
//!   2. 检测到挂载点，切换到 tmpfs 的根 Dentry
//!   3. 在 tmpfs 中查找 "file"
//! ```
//!
//! # 相对路径处理
//!
//! 相对路径基于当前工作目录（cwd）：
//!
//! ```rust
//! // 当前工作目录: /home/user
//! vfs_lookup("file.txt")?;      // → /home/user/file.txt
//! vfs_lookup("../other")?;      // → /home/other
//! ```
//!
//! # 使用示例
//!
//! ## 基本路径查找
//!
//! ```rust
//! use vfs::vfs_lookup;
//!
//! // 查找绝对路径
//! let dentry = vfs_lookup("/etc/passwd")?;
//!
//! // 查找相对路径（基于当前工作目录）
//! let dentry = vfs_lookup("file.txt")?;
//! ```
//!
//! ## 路径解析和规范化
//!
//! ```rust
//! use vfs::{parse_path, normalize_path, PathComponent};
//!
//! // 解析路径
//! let components = parse_path("/home/../etc");
//! // → [Root, Normal("home"), Parent, Normal("etc")]
//!
//! // 规范化路径
//! let normalized = normalize_path("/home/../etc");
//! // → "/etc"
//! ```
//!
//! ## 分割路径
//!
//! ```rust
//! use vfs::split_path;
//!
//! // 分割为目录和文件名
//! let (dir, name) = split_path("/etc/passwd")?;
//! // dir = "/etc", name = "passwd"
//!
//! let (dir, name) = split_path("/file")?;
//! // dir = "/", name = "file"
//! ```
//!
//! ## 符号链接操作
//!
//! ```rust
//! use vfs::{vfs_lookup, vfs_lookup_no_follow};
//!
//! // 创建符号链接 /link → /target
//! parent.inode.symlink("link", "/target")?;
//!
//! // 跟随符号链接
//! let target = vfs_lookup("/link")?;  // 返回 /target
//!
//! // 不跟随符号链接
//! let link = vfs_lookup_no_follow("/link")?;  // 返回 /link
//! let link_target = link.inode.readlink()?;   // 读取链接目标
//! ```

use crate::kernel::current_task;
use crate::vfs::{Dentry, FsError, InodeType, get_root_dentry};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

const MAX_SYMLINK_DEPTH: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathComponent {
    Root,           // "/"
    Current,        // "."
    Parent,         // ".."
    Normal(String), // 正常的文件名
}

/// 将路径字符串解析为组件列表
///
/// 参数：
///     - path: 待解析的路径字符串（支持绝对路径和相对路径）
///
/// 返回：路径组件向量，包含 Root、Current、Parent 或 Normal 组件
pub fn parse_path(path: &str) -> Vec<PathComponent> {
    let mut components = Vec::new();

    // 绝对路径以 Root 开始
    if path.starts_with('/') {
        components.push(PathComponent::Root);
    }

    // 分割路径并解析每个部分
    for part in path.split('/').filter(|s| !s.is_empty()) {
        let component = match part {
            "." => PathComponent::Current,
            ".." => PathComponent::Parent,
            name => PathComponent::Normal(String::from(name)),
        };
        components.push(component);
    }

    components
}

/// 规范化路径（处理 ".." 和 "."）
///
/// 参数：
///     - path: 待规范化的路径字符串
///
/// 返回：规范化后的路径字符串，移除冗余的 `.` 和 `..` 组件
pub fn normalize_path(path: &str) -> String {
    let components = parse_path(path);
    let mut stack: Vec<String> = Vec::new();
    let mut is_absolute = false;

    for component in components {
        match component {
            PathComponent::Root => {
                is_absolute = true;
            }
            PathComponent::Current => {
                // "." 不做任何操作
            }
            PathComponent::Parent => {
                if is_absolute {
                    // 绝对路径：不能越过根目录
                    if !stack.is_empty() {
                        stack.pop();
                    }
                } else {
                    // 相对路径：
                    if let Some(last) = stack.last() {
                        if last == ".." {
                            // 栈顶是 ".." (例如 "/../..")，继续添加 ".."
                            stack.push(String::from(".."));
                        } else {
                            // 栈顶是普通目录 (例如 "a/b/")，弹出一个 (变为 "a/")
                            stack.pop();
                        }
                    } else {
                        // 栈是空的 (即 "/")，添加 ".."
                        stack.push(String::from(".."));
                    }
                }
            }
            PathComponent::Normal(name) => {
                stack.push(name);
            }
        }
    }

    // 构造结果
    if stack.is_empty() {
        if is_absolute {
            String::from("/")
        } else {
            String::from(".")
        }
    } else if is_absolute {
        String::from("/") + &stack.join("/")
    } else {
        stack.join("/")
    }
}

/// 将路径分割为目录部分和文件名部分
///
/// 参数：
///     - path: 待分割的路径字符串
///
/// 返回：Ok((目录, 文件名)) 分割成功；Err(FsError::InvalidArgument) 路径以斜杠结尾或文件名为空
pub fn split_path(path: &str) -> Result<(String, String), FsError> {
    // 如果路径以斜杠结尾，说明是目录而非文件，返回错误
    if path.ends_with('/') && path.len() > 1 {
        return Err(FsError::InvalidArgument);
    }

    // 先规范化路径，处理多余的斜杠和 . / ..
    let normalized = normalize_path(path);

    if let Some(pos) = normalized.rfind('/') {
        let dir = if pos == 0 {
            String::from("/")
        } else {
            String::from(&normalized[..pos])
        };
        let filename = String::from(&normalized[pos + 1..]);

        if filename.is_empty() {
            return Err(FsError::InvalidArgument);
        }

        Ok((dir, filename))
    } else {
        // 相对路径，使用当前目录
        Ok((String::from("."), String::from(normalized)))
    }
}

/// 将路径字符串解析为 Dentry（支持绝对/相对路径、符号链接解析）
///
/// 参数：
///     - path: 文件或目录路径（绝对路径从根目录开始，相对路径从当前工作目录开始）
///
/// 返回：`Ok(Arc<Dentry>)` 路径对应的目录项；`Err(FsError::NotFound)` 路径不存在；`Err(FsError::NotDirectory)` 中间组件不是目录
pub fn vfs_lookup(path: &str) -> Result<Arc<Dentry>, FsError> {
    let components = parse_path(path);

    // 确定起始 dentry
    let current_dentry = if components.first() == Some(&PathComponent::Root) {
        // 绝对路径：从根目录开始
        get_root_dentry()?
    } else {
        // 相对路径：从当前工作目录开始
        get_cur_dir()?
    };

    vfs_walk(current_dentry, components, true)
}

/// 从指定的 base dentry 开始解析路径。
///
/// 如果 `path` 是绝对路径（以'/'开头），此函数会忽略路径中的根组件("/")，
/// 并从传入的 `base` dentry 开始解析。因此，调用者有责任在处理绝对路径时
/// 提供根 dentry 作为 `base`。
///
/// # 参数
/// - `base`: 开始查找的目录项。
/// - `path`: 要解析的路径字符串。
pub fn vfs_lookup_from(base: Arc<Dentry>, path: &str) -> Result<Arc<Dentry>, FsError> {
    let components: Vec<PathComponent> = parse_path(path)
        .into_iter()
        .filter(|c| *c != PathComponent::Root)
        .collect();
    vfs_walk(base, components, true)
}

/// 解析单个路径组件，处理 `.`、`..`、普通文件名和符号链接
fn resolve_component(base: Arc<Dentry>, component: PathComponent) -> Result<Arc<Dentry>, FsError> {
    match component {
        PathComponent::Root => {
            // 已经在根目录，无需操作
            get_root_dentry()
        }
        PathComponent::Current => {
            // "." 表示当前目录
            Ok(base)
        }
        PathComponent::Parent => {
            // ".." 表示父目录
            match base.parent() {
                Some(parent) => check_mount_point(parent),
                None => Ok(base), // 根目录的父目录是自己
            }
        }
        PathComponent::Normal(name) => {
            // 正常文件名：查找子项

            // 1. 先检查 dentry 缓存
            if let Some(child) = base.lookup_child(&name) {
                // 即使缓存命中，也要检查挂载点（可能后来挂载了）
                return check_mount_point(child);
            }

            // 2. 缓存未命中，通过 inode 查找
            let child_inode = base.inode.lookup(&name)?;

            // 3. 创建新的 dentry 并加入缓存
            let child_dentry = Dentry::new(name.clone(), child_inode);
            base.add_child(child_dentry.clone());

            // 4. 加入全局缓存
            crate::vfs::DENTRY_CACHE.insert(&child_dentry);

            // 5. 检查是否有挂载点
            check_mount_point(child_dentry)
        }
    }
}

fn vfs_walk(
    mut current_dentry: Arc<Dentry>,
    mut components: Vec<PathComponent>,
    follow_last_symlink: bool,
) -> Result<Arc<Dentry>, FsError> {
    let mut i = 0usize;
    let mut symlink_depth = 0usize;

    while i < components.len() {
        let component = components[i].clone();
        let is_last = i + 1 == components.len();

        current_dentry = resolve_component(current_dentry, component)?;

        let inode_type = current_dentry.inode.metadata()?.inode_type;
        if inode_type == InodeType::Symlink && (follow_last_symlink || !is_last) {
            if symlink_depth >= MAX_SYMLINK_DEPTH {
                return Err(FsError::TooManySymlinks);
            }
            symlink_depth += 1;

            let target = current_dentry.inode.readlink()?;

            // 需要把“符号链接目标”替换到当前路径中，并继续解析剩余组件。
            // 目标为绝对路径时从全局 root 开始；相对路径时从链接所在目录开始。
            current_dentry = if target.starts_with('/') {
                get_root_dentry()?
            } else {
                match current_dentry.parent() {
                    Some(parent) => parent,
                    None => get_root_dentry()?,
                }
            };

            let mut target_components = parse_path(&target);
            let mut remaining = components.split_off(i + 1);
            target_components.append(&mut remaining);
            components = target_components;
            i = 0;
            continue;
        }

        i += 1;
    }

    Ok(current_dentry)
}

/// 检查给定的 dentry 是否有挂载点，如果有则返回挂载点的根 dentry
fn check_mount_point(dentry: Arc<Dentry>) -> Result<Arc<Dentry>, FsError> {
    // 快速路径：检查 dentry 本地缓存
    if let Some(mounted_root) = dentry.get_mount() {
        return Ok(mounted_root);
    }

    // 慢速路径：查找挂载表（首次访问或缓存失效）
    let full_path = dentry.full_path();
    if let Some(mount_point) = crate::vfs::MOUNT_TABLE.find_mount(&full_path) {
        if mount_point.mount_path == full_path {
            // 更新 dentry 的挂载缓存
            dentry.set_mount(&mount_point.root);
            return Ok(mount_point.root.clone());
        }
    }

    Ok(dentry)
}

/// 获取当前任务的工作目录
fn get_cur_dir() -> Result<Arc<Dentry>, FsError> {
    current_task()
        .lock()
        .fs
        .lock()
        .cwd
        .clone()
        .ok_or(FsError::NotSupported)
}

/// 查找路径但不跟随最后一个符号链接
///
/// # 参数
/// * `path` - 要查找的路径
///
/// # 返回值
/// * `Ok(dentry)` - 找到的 dentry（如果最后一个组件是符号链接，返回链接本身）
/// * `Err(FsError)` - 查找失败
///
/// # 行为
/// 与 `vfs_lookup` 类似，但最后一个路径组件如果是符号链接，
/// 不会跟随它，而是直接返回链接文件的 dentry。
/// 路径中间的符号链接仍然会被跟随。
pub fn vfs_lookup_no_follow(path: &str) -> Result<Arc<Dentry>, FsError> {
    let components = parse_path(path);

    if components.is_empty() {
        return Err(FsError::InvalidArgument);
    }

    // 确定起始 dentry
    let current_dentry = if components.first() == Some(&PathComponent::Root) {
        get_root_dentry()?
    } else {
        get_cur_dir()?
    };

    vfs_walk(current_dentry, components, false)
}

/// 从指定的 base dentry 开始查找路径，但不跟随最后一个符号链接。
///
/// 用于 at 系列系统调用的相对路径场景，避免对 base_dentry.full_path() 做字符串拼接
/// 造成挂载点 root dentry（full_path() == "/"）的解析错误。
pub fn vfs_lookup_no_follow_from(base: Arc<Dentry>, path: &str) -> Result<Arc<Dentry>, FsError> {
    let components: Vec<PathComponent> = parse_path(path)
        .into_iter()
        .filter(|c| *c != PathComponent::Root)
        .collect();
    vfs_walk(base, components, false)
}
