use crate::kernel::current_task;
use crate::vfs::{Dentry, FsError, get_root_dentry};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

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
/// 返回：Ok(Arc<Dentry>) 路径对应的目录项；Err(FsError::NotFound) 路径不存在；Err(FsError::NotDirectory) 中间组件不是目录
pub fn vfs_lookup(path: &str) -> Result<Arc<Dentry>, FsError> {
    let components = parse_path(path);

    // 确定起始 dentry
    let mut current_dentry = if components.first() == Some(&PathComponent::Root) {
        // 绝对路径：从根目录开始
        get_root_dentry()?
    } else {
        // 相对路径：从当前工作目录开始
        get_cur_dir()?
    };

    // 逐个解析路径组件
    for component in components {
        current_dentry = resolve_component(current_dentry, component)?;
    }

    Ok(current_dentry)
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
    let components = parse_path(path);
    let mut current_dentry = base;

    for component in components {
        if component == PathComponent::Root {
            continue;
        }
        current_dentry = resolve_component(current_dentry, component)?;
    }

    Ok(current_dentry)
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
                Some(parent) => Ok(parent),
                None => Ok(base), // 根目录的父目录是自己
            }
        }
        PathComponent::Normal(name) => {
            // 正常文件名：查找子项

            // 1. 先检查 dentry 缓存
            if let Some(child) = base.lookup_child(&name) {
                return Ok(child);
            }

            // 2. 缓存未命中，通过 inode 查找
            let child_inode = base.inode.lookup(&name)?;

            // 3. 创建新的 dentry 并加入缓存
            let child_dentry = Dentry::new(name.clone(), child_inode);
            base.add_child(child_dentry.clone());

            // 4. 加入全局缓存
            crate::vfs::DENTRY_CACHE.insert(&child_dentry);

            Ok(child_dentry)
        }
    }
}

/// 获取当前任务的工作目录
fn get_cur_dir() -> Result<Arc<Dentry>, FsError> {
    current_task()
        .lock()
        .cwd
        .clone()
        .ok_or(FsError::NotSupported)
}
