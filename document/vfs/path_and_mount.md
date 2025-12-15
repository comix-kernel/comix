# 路径解析与挂载管理

## 概述

本文档详细介绍 VFS 子系统的路径解析机制和挂载管理功能。路径解析将字符串路径转换为 Dentry 对象，支持绝对路径、相对路径、符号链接等；挂载管理实现多文件系统共存，支持挂载点栈和动态挂载/卸载。

## 路径解析

### 核心概念

路径解析是将字符串路径（如 `/etc/passwd`）转换为 Dentry 对象的过程。VFS 支持：
- **绝对路径**: 以 `/` 开头，从根目录解析
- **相对路径**: 不以 `/` 开头，从当前工作目录解析
- **特殊组件**: `.` (当前目录) 和 `..` (父目录)
- **符号链接**: 自动跟随符号链接（可选）

### 路径组件

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathComponent {
    Root,           // "/"
    Current,        // "."
    Parent,         // ".."
    Normal(String), // 普通文件名
}
```

### parse_path - 路径解析

将路径字符串分解为组件列表：

```rust
pub fn parse_path(path: &str) -> Vec<PathComponent> {
    let mut components = Vec::new();
    
    // 绝对路径以 Root 开始
    if path.starts_with('/') {
        components.push(PathComponent::Root);
    }
    
    // 分割并解析每个部分
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
```

**示例**:
```rust
parse_path("/etc/passwd")       // [Root, Normal("etc"), Normal("passwd")]
parse_path("../foo/./bar")      // [Parent, Normal("foo"), Current, Normal("bar")]
parse_path("/")                 // [Root]
```

### normalize_path - 路径规范化

处理 `.` 和 `..`，生成规范化路径：

```rust
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
                    // 相对路径：处理 ".."
                    if let Some(last) = stack.last() {
                        if last == ".." {
                            stack.push(String::from(".."));
                        } else {
                            stack.pop();
                        }
                    } else {
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
```

**示例**:
```rust
normalize_path("/a/b/../c/./d")     // "/a/c/d"
normalize_path("../../foo")         // "../../foo"
normalize_path("/a/b/../../")       // "/"
normalize_path("./foo/./bar")       // "foo/bar"
```

### split_path - 路径分割

将路径分割为目录部分和文件名：

```rust
pub fn split_path(path: &str) -> Result<(String, String), FsError> {
    // 路径以斜杠结尾表示目录
    if path.ends_with('/') && path.len() > 1 {
        return Err(FsError::InvalidArgument);
    }
    
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
        // 相对路径
        Ok((String::from("."), String::from(normalized)))
    }
}
```

**示例**:
```rust
split_path("/etc/passwd")    // Ok(("/etc", "passwd"))
split_path("/passwd")        // Ok(("/", "passwd"))
split_path("foo/bar")        // Ok(("foo", "bar"))
split_path("file.txt")       // Ok((".", "file.txt"))
```

### vfs_lookup - 路径查找

将路径转换为 Dentry，这是路径解析的核心函数：

```rust
pub fn vfs_lookup(path: &str) -> Result<Arc<Dentry>, FsError> {
    let components = parse_path(path);
    
    // 确定起始 dentry
    let mut current_dentry = if components.first() == Some(&PathComponent::Root) {
        get_root_dentry()?  // 绝对路径：从根目录开始
    } else {
        get_cur_dir()?      // 相对路径：从当前工作目录开始
    };
    
    // 逐个解析路径组件
    for component in components {
        current_dentry = resolve_component(current_dentry, component)?;
    }
    
    Ok(current_dentry)
}
```

### resolve_component - 组件解析

解析单个路径组件，包括缓存查找、Inode lookup、挂载点检查：

```rust
fn resolve_component(base: Arc<Dentry>, component: PathComponent) 
    -> Result<Arc<Dentry>, FsError> {
    match component {
        PathComponent::Root => {
            get_root_dentry()
        }
        PathComponent::Current => {
            Ok(base)
        }
        PathComponent::Parent => {
            match base.parent() {
                Some(parent) => check_mount_point(parent),
                None => Ok(base),  // 根目录的父目录是自己
            }
        }
        PathComponent::Normal(name) => {
            // 1. 先检查 dentry 缓存
            if let Some(child) = base.lookup_child(&name) {
                return check_mount_point(child);
            }
            
            // 2. 缓存未命中，通过 inode 查找
            let child_inode = base.inode.lookup(&name)?;
            
            // 3. 创建新的 dentry 并加入缓存
            let child_dentry = Dentry::new(name.clone(), child_inode);
            base.add_child(child_dentry.clone());
            
            // 4. 加入全局缓存
            DENTRY_CACHE.insert(&child_dentry);
            
            // 5. 检查是否有挂载点
            check_mount_point(child_dentry)
        }
    }
}
```

### check_mount_point - 挂载点检查

检查 Dentry 是否是挂载点，如果是则返回挂载的根 Dentry：

```rust
fn check_mount_point(dentry: Arc<Dentry>) -> Result<Arc<Dentry>, FsError> {
    // 快速路径：检查 dentry 本地缓存
    if let Some(mounted_root) = dentry.get_mount() {
        return Ok(mounted_root);
    }
    
    // 慢速路径：查找挂载表
    let full_path = dentry.full_path();
    if let Some(mount_point) = MOUNT_TABLE.find_mount(&full_path) {
        if mount_point.mount_path == full_path {
            // 更新 dentry 的挂载缓存
            dentry.set_mount(&mount_point.root);
            return Ok(mount_point.root.clone());
        }
    }
    
    Ok(dentry)
}
```

### vfs_lookup_from - 从指定 Dentry 查找

从给定的 base Dentry 开始查找路径：

```rust
pub fn vfs_lookup_from(base: Arc<Dentry>, path: &str) 
    -> Result<Arc<Dentry>, FsError> {
    let components = parse_path(path);
    let mut current_dentry = base;
    
    for component in components {
        if component == PathComponent::Root {
            continue;  // 忽略根组件
        }
        current_dentry = resolve_component(current_dentry, component)?;
    }
    
    Ok(current_dentry)
}
```

### vfs_lookup_no_follow - 不跟随符号链接

查找路径但不跟随最后一个组件的符号链接（用于 lstat、unlink 等）：

```rust
pub fn vfs_lookup_no_follow(path: &str) -> Result<Arc<Dentry>, FsError> {
    let components = parse_path(path);
    
    if components.is_empty() {
        return Err(FsError::InvalidArgument);
    }
    
    let mut current_dentry = if components.first() == Some(&PathComponent::Root) {
        get_root_dentry()?
    } else {
        get_cur_dir()?
    };
    
    if components.len() == 1 && components[0] == PathComponent::Root {
        return Ok(current_dentry);
    }
    
    // 解析除最后一个组件外的所有组件
    let len = components.len();
    for i in 0..len - 1 {
        current_dentry = resolve_component(current_dentry, components[i].clone())?;
    }
    
    // 解析最后一个组件，但不跟随符号链接
    let last_component = &components[len - 1];
    match last_component {
        PathComponent::Root => get_root_dentry(),
        PathComponent::Current => Ok(current_dentry),
        PathComponent::Parent => {
            match current_dentry.parent() {
                Some(parent) => Ok(parent),
                None => Ok(current_dentry),
            }
        }
        PathComponent::Normal(name) => {
            // 查找但不跟随符号链接
            if let Some(child) = current_dentry.lookup_child(name) {
                return Ok(child);
            }
            
            let child_inode = current_dentry.inode.lookup(name)?;
            let child_dentry = Dentry::new(name.clone(), child_inode);
            current_dentry.add_child(child_dentry.clone());
            DENTRY_CACHE.insert(&child_dentry);
            
            Ok(child_dentry)
        }
    }
}
```

## 挂载管理

### 核心概念

挂载管理允许多个文件系统共存于同一目录树中。挂载点是文件系统的接入点，访问挂载点下的路径时会自动切换到挂载的文件系统。

#### 关键特性

- **挂载点栈**: 同一路径可以多次挂载，最后挂载的文件系统覆盖之前的
- **最长前缀匹配**: 查找挂载点时使用最长前缀匹配算法
- **动态挂载/卸载**: 支持运行时挂载和卸载文件系统

### MountFlags - 挂载标志

```rust
bitflags! {
    pub struct MountFlags: u32 {
        const READ_ONLY  = 1 << 0;  // 只读挂载
        const NO_EXEC    = 1 << 1;  // 禁止执行
        const NO_SUID    = 1 << 2;  // 忽略 SUID/SGID 位
        const SYNC       = 1 << 3;  // 同步写入
        const NO_DEV     = 1 << 4;  // 禁止设备文件
    }
}
```

### MountPoint - 挂载点结构

```rust
pub struct MountPoint {
    /// 挂载的文件系统
    pub fs: Arc<dyn FileSystem>,
    
    /// 挂载点的根 dentry
    pub root: Arc<Dentry>,
    
    /// 挂载标志
    pub flags: MountFlags,
    
    /// 设备路径 (如果有)
    pub device: Option<String>,
    
    /// 挂载路径
    pub mount_path: String,
}

impl MountPoint {
    pub fn new(fs: Arc<dyn FileSystem>, mount_path: String, 
               flags: MountFlags, device: Option<String>) -> Arc<Self> {
        let root_inode = fs.root_inode();
        let root = Dentry::new(String::from("/"), root_inode);
        
        Arc::new(Self {
            fs,
            root,
            flags,
            device,
            mount_path,
        })
    }
}
```

### MountTable - 挂载表

```rust
pub struct MountTable {
    /// 挂载路径 -> 挂载点栈 (最后一个是当前可见的)
    mounts: SpinLock<BTreeMap<String, Vec<Arc<MountPoint>>>>,
}

lazy_static! {
    pub static ref MOUNT_TABLE: MountTable = MountTable::new();
}
```

### mount - 挂载文件系统

```rust
impl MountTable {
    pub fn mount(&self, fs: Arc<dyn FileSystem>, path: &str, 
                 flags: MountFlags, device: Option<String>) 
        -> Result<(), FsError> {
        let normalized_path = normalize_path(path);
        
        // 创建挂载点
        let mount_point = MountPoint::new(fs, normalized_path.clone(), 
                                          flags, device);
        
        // 添加到挂载栈
        let mut mounts = self.mounts.lock();
        mounts.entry(normalized_path.clone())
            .or_insert_with(Vec::new)
            .push(mount_point.clone());
        
        // 更新 dentry 缓存中的挂载信息
        if let Some(dentry) = DENTRY_CACHE.lookup(&normalized_path) {
            dentry.set_mount(&mount_point.root);
        }
        
        Ok(())
    }
}
```

### umount - 卸载文件系统

```rust
impl MountTable {
    pub fn umount(&self, path: &str) -> Result<(), FsError> {
        let normalized_path = normalize_path(path);
        
        // 不允许卸载根文件系统
        if normalized_path == "/" {
            return Err(FsError::NotSupported);
        }
        
        let mut mounts = self.mounts.lock();
        let stack = mounts.get_mut(&normalized_path)
            .ok_or(FsError::NotFound)?;
        
        // 弹出栈顶的挂载点
        let mount_point = stack.pop().ok_or(FsError::NotFound)?;
        
        // 如果栈为空，移除整个条目
        if stack.is_empty() {
            mounts.remove(&normalized_path);
        }
        
        drop(mounts);  // 释放锁
        
        // 同步文件系统
        mount_point.fs.sync()?;
        
        // 执行卸载清理
        mount_point.fs.umount()?;
        
        // 更新 dentry 缓存
        if let Some(dentry) = DENTRY_CACHE.lookup(&normalized_path) {
            let mounts = self.mounts.lock();
            if let Some(stack) = mounts.get(&normalized_path) {
                if let Some(underlying_mount) = stack.last() {
                    dentry.set_mount(&underlying_mount.root);
                } else {
                    dentry.clear_mount();
                }
            } else {
                dentry.clear_mount();
            }
        }
        
        Ok(())
    }
}
```

### find_mount - 查找挂载点

使用最长前缀匹配查找挂载点：

```rust
impl MountTable {
    pub fn find_mount(&self, path: &str) -> Option<Arc<MountPoint>> {
        let normalized_path = normalize_path(path);
        let mounts = self.mounts.lock();
        
        // 查找最长匹配的挂载点
        let mut best_match = None;
        let mut best_len = 0;
        
        for (mount_path, stack) in mounts.iter() {
            if normalized_path.starts_with(mount_path) 
                && mount_path.len() > best_len {
                // 返回栈顶的挂载点 (当前可见的)
                if let Some(mp) = stack.last() {
                    best_match = Some(mp.clone());
                    best_len = mount_path.len();
                }
            }
        }
        
        best_match
    }
}
```

**示例**:
```rust
// 挂载情况:
// "/" -> tmpfs
// "/mnt" -> fat32
// "/mnt/data" -> ext4

find_mount("/etc/passwd")       // Some(tmpfs at "/")
find_mount("/mnt/config")       // Some(fat32 at "/mnt")
find_mount("/mnt/data/file")    // Some(ext4 at "/mnt/data")
```

### root_mount - 获取根挂载点

```rust
impl MountTable {
    pub fn root_mount(&self) -> Option<Arc<MountPoint>> {
        self.mounts.lock()
            .get("/")
            .and_then(|stack| stack.last())
            .cloned()
    }
}

pub fn get_root_dentry() -> Result<Arc<Dentry>, FsError> {
    MOUNT_TABLE.root_mount()
        .map(|mp| mp.root.clone())
        .ok_or(FsError::NotSupported)
}
```

### list_mounts - 列出所有挂载点

```rust
impl MountTable {
    pub fn list_mounts(&self) -> Vec<(String, String)> {
        let mounts = self.mounts.lock();
        mounts.iter()
            .flat_map(|(path, stack)| {
                stack.iter()
                    .map(|mp| (path.clone(), String::from(mp.fs.fs_type())))
            })
            .collect()
    }
}
```

## 使用示例

### 路径查找

```rust
// 绝对路径查找
let dentry = vfs_lookup("/etc/passwd")?;

// 相对路径查找
let dentry = vfs_lookup("../foo/bar")?;

// 查找但不跟随符号链接
let dentry = vfs_lookup_no_follow("/path/to/symlink")?;

// 从指定 dentry 查找
let base = vfs_lookup("/mnt")?;
let dentry = vfs_lookup_from(base, "data/file.txt")?;
```

### 挂载文件系统

```rust
// 创建文件系统实例
let fs: Arc<dyn FileSystem> = create_tmpfs()?;

// 挂载到 /tmp
MOUNT_TABLE.mount(
    fs,
    "/tmp",
    MountFlags::empty(),
    None
)?;

// 访问挂载点下的文件
let dentry = vfs_lookup("/tmp/test.txt")?;

// 卸载
MOUNT_TABLE.umount("/tmp")?;
```

### 挂载点栈

```rust
// 第一次挂载
let fs1 = create_tmpfs()?;
MOUNT_TABLE.mount(fs1, "/mnt", MountFlags::empty(), None)?;

// 第二次挂载 (覆盖)
let fs2 = create_fat32()?;
MOUNT_TABLE.mount(fs2, "/mnt", MountFlags::empty(), 
                  Some(String::from("/dev/sda1")))?;

// 访问 /mnt 会使用 fs2
let dentry = vfs_lookup("/mnt")?;

// 卸载第二次挂载
MOUNT_TABLE.umount("/mnt")?;

// 现在访问 /mnt 会使用 fs1
```

### 多级挂载

```rust
// 挂载根文件系统
let tmpfs = create_tmpfs()?;
MOUNT_TABLE.mount(tmpfs, "/", MountFlags::empty(), None)?;

// 挂载 /mnt
let fat32 = create_fat32()?;
MOUNT_TABLE.mount(fat32, "/mnt", MountFlags::empty(), 
                  Some(String::from("/dev/sda1")))?;

// 挂载 /mnt/data
let ext4 = create_ext4()?;
MOUNT_TABLE.mount(ext4, "/mnt/data", MountFlags::empty(), 
                  Some(String::from("/dev/sda2")))?;

// 路径解析会自动切换到对应的文件系统
vfs_lookup("/etc/passwd")       // 在 tmpfs 中查找
vfs_lookup("/mnt/config")       // 在 fat32 中查找
vfs_lookup("/mnt/data/file")    // 在 ext4 中查找
```

## 性能优化

### 缓存策略

1. **Dentry 全局缓存**: 避免重复路径解析
2. **Dentry 树缓存**: 父子关系缓存，加速相对路径查找
3. **挂载点本地缓存**: Dentry 缓存挂载点信息，避免每次查挂载表

### 路径解析优化

1. **短路径优先**: 尽量使用绝对路径，避免复杂的 `../..` 等相对路径
2. **批量操作**: 在同一目录下操作多个文件时，先查找目录 Dentry，再使用 `vfs_lookup_from`
3. **缓存预热**: 启动时预加载常用路径到缓存

## 最佳实践

### 路径处理

1. **总是规范化路径**: 使用 `normalize_path` 处理用户输入
2. **检查路径有效性**: 使用 `split_path` 验证路径格式
3. **选择合适的查找函数**:
   - 普通查找: `vfs_lookup`
   - 不跟随符号链接: `vfs_lookup_no_follow`
   - 从指定位置查找: `vfs_lookup_from`

### 挂载管理

1. **先挂载根目录**: 系统启动时先挂载根文件系统
2. **检查挂载点**: 挂载前确保目录存在
3. **优雅卸载**: 卸载前确保没有进程使用该文件系统
4. **错误处理**: 挂载/卸载失败时正确清理资源

### 安全考虑

1. **路径越界检查**: 防止 `../../../` 越过根目录
2. **权限验证**: 检查用户是否有权限访问路径
3. **符号链接循环**: 限制符号链接解析深度（当前未实现）

## 常见问题

### Q: 绝对路径和相对路径有什么区别?

A:
- **绝对路径**: 以 `/` 开头，从根目录解析，如 `/etc/passwd`
- **相对路径**: 不以 `/` 开头，从当前工作目录解析，如 `../foo/bar`

### Q: 挂载点栈有什么用?

A:
支持同一路径多次挂载，常用于容器技术。最后挂载的文件系统覆盖之前的，卸载后恢复为下层挂载。

### Q: 最长前缀匹配如何工作?

A:
访问 `/mnt/data/file` 时，如果 `/`、`/mnt` 和 `/mnt/data` 都是挂载点，则选择 `/mnt/data`（最长匹配）。

### Q: 如何实现符号链接?

A:
1. 创建 `InodeType::Symlink` 类型的 Inode
2. 实现 `readlink()` 返回目标路径
3. 路径解析时检测到符号链接，递归调用 `vfs_lookup` 解析目标路径

### Q: 为什么需要 vfs_lookup_no_follow?

A:
某些操作需要操作符号链接本身而不是目标，如:
- `unlink`: 删除符号链接文件
- `lstat`: 获取符号链接的元数据
- `readlink`: 读取符号链接目标

## 相关资源

### 源代码位置

- **路径解析**: `os/src/vfs/path.rs`
- **挂载管理**: `os/src/vfs/mount.rs`
- **Dentry 缓存**: `os/src/vfs/dentry.rs`

### 参考文档

- [VFS 整体架构](architecture.md)
- [Inode 与 Dentry](inode_and_dentry.md)
- [File 与 FDTable](file_and_fdtable.md)
- [使用指南](usage.md)
