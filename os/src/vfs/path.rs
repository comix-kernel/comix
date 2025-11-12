use crate::vfs::{Dentry, FsError};
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

pub fn split_path(path: &str) -> Result<(String, String), FsError> {
    if let Some(pos) = path.rfind('/') {
        let dir = if pos == 0 {
            String::from("/")
        } else {
            String::from(&path[..pos])
        };
        let filename = String::from(&path[pos + 1..]);

        if filename.is_empty() {
            return Err(FsError::InvalidArgument);
        }

        Ok((dir, filename))
    } else {
        // 相对路径，使用当前目录
        Ok((String::from("."), String::from(path)))
    }
}

// TODO: 实现VFS 路径解析，将路径字符串解析为 Dentry
/// VFS 路径解析
pub fn vfs_lookup(path: &str) -> Result<Arc<Dentry>, FsError> {
    unimplemented!()
}

fn resolve_component(base: Arc<Dentry>, component: PathComponent) -> Result<Arc<Dentry>, FsError> {
    unimplemented!()
}

fn get_root_dentry() -> Result<Arc<Dentry>, FsError> {
    unimplemented!()
}

fn get_cur_dir() -> Result<Arc<Dentry>, FsError> {
    unimplemented!()
}
