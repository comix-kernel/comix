# Inode 与 Dentry

## 概述

本文档详细介绍 VFS 子系统的存储层 (Inode) 和路径层 (Dentry) 的设计与实现。Inode 提供无状态的文件存储访问接口，Dentry 管理文件系统的目录树结构和路径缓存。

## Inode - 存储层接口

### 核心概念

Inode (Index Node) 是文件系统中文件或目录的底层表示，提供无状态的随机访问能力。与会话层的 File trait 不同，Inode 的所有读写方法都携带 offset 参数，不维护任何会话状态。

#### Inode 的职责

- **数据访问**: `read_at(offset, buf)` 和 `write_at(offset, buf)` 提供指定偏移量的读写
- **目录操作**: `lookup(name)` 查找子项，`create/mkdir/unlink/rmdir` 管理目录结构
- **元数据管理**: `metadata()` 获取文件信息，`truncate/chmod/chown` 修改文件属性
- **符号链接**: `symlink/readlink` 创建和读取符号链接
- **设备文件**: `mknod` 创建设备文件节点
- **同步**: `sync()` 将数据刷新到持久化存储

### Inode Trait 定义

```rust
pub trait Inode: Send + Sync + Any {
    // 元数据访问
    fn metadata(&self) -> Result<InodeMetadata, FsError>;
    
    // 数据访问 (无状态,携带 offset)
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError>;
    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError>;
    
    // 目录操作
    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, FsError>;
    fn create(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError>;
    fn mkdir(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError>;
    fn unlink(&self, name: &str) -> Result<(), FsError>;
    fn rmdir(&self, name: &str) -> Result<(), FsError>;
    fn readdir(&self) -> Result<Vec<DirEntry>, FsError>;
    
    // 链接操作
    fn symlink(&self, name: &str, target: &str) -> Result<Arc<dyn Inode>, FsError>;
    fn link(&self, name: &str, target: &Arc<dyn Inode>) -> Result<(), FsError>;
    fn readlink(&self) -> Result<String, FsError>;
    
    // 文件管理
    fn truncate(&self, size: usize) -> Result<(), FsError>;
    fn sync(&self) -> Result<(), FsError>;
    fn chmod(&self, mode: FileMode) -> Result<(), FsError>;
    fn chown(&self, uid: u32, gid: u32) -> Result<(), FsError>;
    fn set_times(&self, atime: Option<TimeSpec>, mtime: Option<TimeSpec>) 
        -> Result<(), FsError>;
    
    // 设备文件
    fn mknod(&self, name: &str, mode: FileMode, dev: u64) 
        -> Result<Arc<dyn Inode>, FsError>;
    
    // Dentry 关联 (可选)
    fn set_dentry(&self, _dentry: Weak<Dentry>) {}
    fn get_dentry(&self) -> Option<Arc<Dentry>> { None }
    
    // 向下转型支持
    fn as_any(&self) -> &dyn Any;
}
```

### InodeMetadata 结构

```rust
pub struct InodeMetadata {
    pub inode_no: usize,        // Inode 编号
    pub inode_type: InodeType,  // 文件类型
    pub mode: FileMode,         // 权限位
    pub uid: u32,               // 用户 ID
    pub gid: u32,               // 组 ID
    pub size: usize,            // 文件大小 (字节)
    pub atime: TimeSpec,        // 访问时间
    pub mtime: TimeSpec,        // 修改时间
    pub ctime: TimeSpec,        // 状态改变时间
    pub nlinks: usize,          // 硬链接数
    pub blocks: usize,          // 占用的块数 (512B 为单位)
    pub rdev: u64,              // 设备号 (仅设备文件有效)
}
```

### 文件类型 InodeType

```rust
pub enum InodeType {
    File,         // 普通文件
    Directory,    // 目录
    Symlink,      // 符号链接
    CharDevice,   // 字符设备
    BlockDevice,  // 块设备
    Fifo,         // 命名管道
    Socket,       // 套接字
}
```

### 文件权限 FileMode

```rust
bitflags! {
    pub struct FileMode: u32 {
        // 文件类型掩码
        const S_IFMT   = 0o170000;
        const S_IFREG  = 0o100000;  // 普通文件
        const S_IFDIR  = 0o040000;  // 目录
        const S_IFLNK  = 0o120000;  // 符号链接
        const S_IFCHR  = 0o020000;  // 字符设备
        const S_IFBLK  = 0o060000;  // 块设备
        
        // 用户权限
        const S_IRUSR  = 0o400;     // 用户读
        const S_IWUSR  = 0o200;     // 用户写
        const S_IXUSR  = 0o100;     // 用户执行
        
        // 组权限
        const S_IRGRP  = 0o040;
        const S_IWGRP  = 0o020;
        const S_IXGRP  = 0o010;
        
        // 其他用户权限
        const S_IROTH  = 0o004;
        const S_IWOTH  = 0o002;
        const S_IXOTH  = 0o001;
        
        // 特殊位
        const S_ISUID  = 0o4000;    // Set UID
        const S_ISGID  = 0o2000;    // Set GID
        const S_ISVTX  = 0o1000;    // Sticky bit
    }
}
```

### Inode 实现示例

不同文件系统需要实现自己的 Inode 类型。以下是 tmpfs (内存文件系统) 的简化示例:

```rust
pub struct TmpfsInode {
    metadata: SpinLock<InodeMetadata>,
    data: SpinLock<Vec<u8>>,                    // 文件数据
    children: SpinLock<BTreeMap<String, Arc<dyn Inode>>>,  // 目录子项
    dentry: SpinLock<Weak<Dentry>>,            // 关联的 Dentry
}

impl Inode for TmpfsInode {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        let data = self.data.lock();
        if offset >= data.len() {
            return Ok(0);
        }
        let len = core::cmp::min(buf.len(), data.len() - offset);
        buf[..len].copy_from_slice(&data[offset..offset + len]);
        Ok(len)
    }
    
    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        let mut data = self.data.lock();
        let end = offset + buf.len();
        if end > data.len() {
            data.resize(end, 0);
        }
        data[offset..end].copy_from_slice(buf);
        
        // 更新元数据
        let mut metadata = self.metadata.lock();
        metadata.size = data.len();
        metadata.mtime = get_current_time();
        
        Ok(buf.len())
    }
    
    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, FsError> {
        self.children.lock()
            .get(name)
            .cloned()
            .ok_or(FsError::NotFound)
    }
    
    // ... 其他方法实现
}
```

## Dentry - 路径层结构

### 核心概念

Dentry (Directory Entry) 是路径组件的缓存，表示目录树中的一个节点。Dentry 缓存文件名到 Inode 的映射，避免重复的路径解析和 Inode 查找。

#### Dentry 的职责

- **路径缓存**: 缓存从根目录到文件的完整路径
- **父子关系**: 维护目录树的层次结构
- **Inode 关联**: 持有对应的 `Arc<dyn Inode>`
- **挂载点标记**: 标识该 Dentry 是否为挂载点

### Dentry 结构

```rust
pub struct Dentry {
    /// 文件名 (不含路径)
    pub name: String,
    
    /// 关联的 inode
    pub inode: Arc<dyn Inode>,
    
    /// 父目录 dentry (弱引用避免循环)
    parent: SpinLock<Weak<Dentry>>,
    
    /// 子 dentry 映射 (文件名 -> dentry)
    children: SpinLock<BTreeMap<String, Arc<Dentry>>>,
    
    /// 如果此 dentry 是挂载点，指向挂载的根 dentry
    mount_point: SpinLock<Option<Weak<Dentry>>>,
}
```

### Dentry 方法

```rust
impl Dentry {
    /// 创建新的 dentry
    pub fn new(name: String, inode: Arc<dyn Inode>) -> Arc<Self>;
    
    /// 设置父 dentry
    pub fn set_parent(&self, parent: &Arc<Dentry>);
    
    /// 获取父 dentry
    pub fn parent(&self) -> Option<Arc<Dentry>>;
    
    /// 查找子 dentry (从缓存)
    pub fn lookup_child(&self, name: &str) -> Option<Arc<Dentry>>;
    
    /// 添加子 dentry
    pub fn add_child(self: &Arc<Self>, child: Arc<Dentry>);
    
    /// 删除子 dentry
    pub fn remove_child(&self, name: &str) -> Option<Arc<Dentry>>;
    
    /// 获取完整路径
    pub fn full_path(&self) -> String;
    
    /// 挂载点操作
    pub fn set_mount(&self, mounted_root: &Arc<Dentry>);
    pub fn clear_mount(&self);
    pub fn get_mount(&self) -> Option<Arc<Dentry>>;
}
```

### 完整路径生成

`full_path()` 方法通过向上遍历父节点生成完整路径:

```rust
pub fn full_path(&self) -> String {
    let mut components = Vec::new();
    let mut current = self as *const Dentry;
    
    loop {
        let dentry = unsafe { &*current };
        
        if dentry.name == "/" {
            break;  // 到达根目录
        }
        
        components.push(dentry.name.clone());
        
        match dentry.parent() {
            Some(parent) => current = Arc::as_ptr(&parent),
            None => break,
        }
    }
    
    components.reverse();
    
    if components.is_empty() {
        String::from("/")
    } else {
        String::from("/") + &components.join("/")
    }
}
```

### 全局 Dentry 缓存

VFS 维护一个全局 Dentry 缓存，加速重复路径查找:

```rust
pub struct DentryCache {
    /// 路径 -> dentry 的弱引用映射
    cache: SpinLock<BTreeMap<String, Weak<Dentry>>>,
}

impl DentryCache {
    /// 查找缓存
    pub fn lookup(&self, path: &str) -> Option<Arc<Dentry>> {
        let cache = self.cache.lock();
        let weak = cache.get(path)?;
        weak.upgrade()  // Weak -> Arc，失败说明已被回收
    }
    
    /// 插入缓存
    pub fn insert(&self, dentry: &Arc<Dentry>) {
        let path = dentry.full_path();
        self.cache.lock().insert(path, Arc::downgrade(dentry));
    }
    
    /// 删除缓存
    pub fn remove(&self, path: &str) {
        self.cache.lock().remove(path);
    }
    
    /// 清空缓存
    pub fn clear(&self) {
        self.cache.lock().clear();
    }
}

// 全局单例
lazy_static! {
    pub static ref DENTRY_CACHE: DentryCache = DentryCache::new();
}
```

## 引用计数与生命周期

### Dentry 引用关系

Dentry 使用 `Arc` 和 `Weak` 智能指针管理生命周期:

```
┌──────────────────────────────────────┐
│  Arc<Dentry("/")>                    │  根目录 (强引用)
│    ├─ name: "/"                      │
│    ├─ parent: Weak::new()            │  根目录无父目录
│    ├─ children: {"etc" -> Arc<...>}  │  强引用子目录
│    └─ inode: Arc<TmpfsInode>         │  强引用 Inode
└──────────┬───────────────────────────┘
           │
           │ Arc (强引用)
           ▼
┌──────────────────────────────────────┐
│  Arc<Dentry("/etc")>                 │
│    ├─ name: "etc"                    │
│    ├─ parent: Weak<Dentry("/")>      │  弱引用父目录 (避免循环)
│    ├─ children: {"passwd" -> Arc}    │
│    └─ inode: Arc<TmpfsInode>         │
└──────────┬───────────────────────────┘
           │
           │ Arc
           ▼
┌──────────────────────────────────────┐
│  Arc<Dentry("/etc/passwd")>          │
│    ├─ parent: Weak<Dentry("/etc")>   │
│    └─ inode: Arc<TmpfsInode>         │
└──────────────────────────────────────┘
```

### 为什么使用 Weak 引用?

1. **避免循环引用**: 父节点持有子节点的 Arc，子节点持有父节点的 Weak，打破循环
2. **自动回收**: 当没有外部引用时，Dentry 自动被释放，无需手动清理
3. **缓存失效**: 全局缓存使用 Weak，不延长 Dentry 生命周期

### Inode 共享 (硬链接)

多个 Dentry 可以共享同一个 Inode，实现硬链接:

```
Dentry("/home/user/file.txt")  ────┐
                                    ├──> Arc<TmpfsInode>
Dentry("/tmp/link_to_file")    ────┘

metadata.nlinks = 2  # 硬链接计数
```

## 缓存一致性

### 缓存更新时机

| 操作 | Dentry 缓存 | Dentry 树 |
|------|-------------|-----------|
| lookup 成功 | 插入 `DENTRY_CACHE.insert()` | 插入父节点 `parent.add_child()` |
| create/mkdir | 插入新 Dentry | 插入父节点 |
| unlink/rmdir | 删除 `DENTRY_CACHE.remove()` | 删除父节点 `parent.remove_child()` |
| rename | 更新路径缓存 | 从旧父节点移除，加入新父节点 |

### 缓存失效策略

#### 自动失效 (Weak 引用)

全局缓存使用 `Weak<Dentry>`,当 Dentry 不再被使用时自动失效:

```rust
let dentry = DENTRY_CACHE.lookup("/tmp/file");  // 返回 None (已被回收)
```

#### 手动失效

文件删除时需要手动清理缓存:

```rust
// sys_unlink 实现
pub fn sys_unlink(path: &str) -> Result<(), FsError> {
    let (dir, name) = split_path(path)?;
    let parent = vfs_lookup(&dir)?;
    
    // 删除 Inode
    parent.inode.unlink(&name)?;
    
    // 删除 Dentry 缓存
    parent.remove_child(&name);
    DENTRY_CACHE.remove(path);
    
    Ok(())
}
```

## DirEntry - 轻量级目录项

`readdir` 系统调用返回轻量级的 `DirEntry`，不持有 Arc 引用:

```rust
pub struct DirEntry {
    pub name: String,           // 文件名
    pub inode_no: usize,        // Inode 编号
    pub inode_type: InodeType,  // 文件类型
}

// 使用示例
let entries = inode.readdir()?;
for entry in entries {
    println!("{:?} {} (inode {})", 
        entry.inode_type, entry.name, entry.inode_no);
}
```

## 最佳实践

### 实现 Inode 时的注意事项

1. **线程安全**: Inode 必须实现 `Send + Sync`，所有可变状态需要用锁保护
2. **错误处理**: 返回准确的 FsError 类型 (NotFound/IsDirectory/PermissionDenied 等)
3. **元数据更新**: write_at/truncate 等操作后更新 mtime/ctime
4. **原子操作**: rename 等操作应该是原子的，使用文件系统级锁保证

### 使用 Dentry 时的注意事项

1. **优先查缓存**: 总是先查 `DENTRY_CACHE.lookup()`，未命中再查 Inode
2. **及时清理**: 文件删除后立即删除缓存，避免访问到已删除的文件
3. **避免长时间持有**: Dentry 的 Arc 不应该在系统调用之外长期持有
4. **挂载点检查**: 路径解析时检查 `check_mount_point()`，自动跟随挂载

### 性能优化建议

1. **批量 readdir**: 一次 readdir 返回所有子项，避免多次 lookup
2. **预加载子项**: 访问目录时可以预先将子项加入 Dentry 树
3. **限制缓存大小**: 如果内存紧张，可以实现 LRU 淘汰策略
4. **异步 I/O**: 对于网络文件系统，Inode 操作可以异步实现

## 常见问题

### Q: Dentry 和 Inode 有什么区别?

A: 
- **Dentry** 是路径层的缓存，可能有多个 Dentry 指向同一个 Inode (硬链接)
- **Inode** 是存储层的实体，代表物理文件，与路径无关
- 删除 Dentry 不影响 Inode，只有当 nlinks 为 0 时 Inode 才被删除

### Q: 为什么 Inode 方法是 `read_at` 而不是 `read`?

A: 
Inode 是无状态的，不维护 offset。多个进程可以共享同一个 Inode，各自有独立的 offset (在 File 层维护)。

### Q: Dentry 缓存会不会无限增长?

A: 
不会。全局缓存使用 `Weak<Dentry>`，当 Dentry 不再被外部引用时，`Weak::upgrade()` 返回 None，缓存自动失效。

### Q: 如何实现符号链接?

A: 
1. 创建 InodeType::Symlink 类型的 Inode
2. 实现 `readlink()` 返回目标路径
3. 路径解析时检测到符号链接，递归解析目标路径

### Q: 硬链接和符号链接有什么区别?

A:
- **硬链接**: 多个 Dentry 共享同一个 Inode，`link()` 操作，删除一个不影响其他
- **符号链接**: 创建新的 Inode，存储目标路径字符串，`symlink()` 操作

## 相关资源

### 源代码位置

- **Inode trait**: `os/src/vfs/inode.rs`
- **Dentry 结构**: `os/src/vfs/dentry.rs`
- **示例实现**: `os/src/fs/tmpfs/` (tmpfs Inode 实现)

### 参考文档

- [VFS 整体架构](architecture.md)
- [File 与 FDTable](file_and_fdtable.md)
- [路径解析与挂载](path_and_mount.md)
