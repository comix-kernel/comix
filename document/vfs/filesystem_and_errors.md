# FileSystem Trait 与错误处理

## 概述

本文档详细介绍 VFS 的 FileSystem trait 接口和错误处理机制。FileSystem trait 定义了文件系统的抽象接口，所有具体的文件系统实现（如 tmpfs、fat32）都必须实现此接口。错误处理部分说明了 VFS 的错误类型系统及其与 POSIX errno 的映射。

## FileSystem Trait

### 核心概念

FileSystem trait 是文件系统的顶层抽象，它定义了文件系统级别的操作，而不是单个文件的操作（那是 Inode 的职责）。

#### FileSystem 的职责

- **提供根 Inode**: 返回文件系统的根目录 Inode
- **同步操作**: 将缓存数据刷新到持久化存储
- **统计信息**: 提供文件系统使用情况统计
- **卸载清理**: 执行卸载前的资源清理

### FileSystem Trait 定义

```rust
pub trait FileSystem: Send + Sync {
    /// 文件系统类型名称
    ///
    /// 返回文件系统类型的静态字符串，如 "tmpfs"、"fat32"、"ext4"
    fn fs_type(&self) -> &'static str;
    
    /// 获取根 inode
    ///
    /// 返回文件系统的根目录 inode，用于挂载时创建根 Dentry
    fn root_inode(&self) -> Arc<dyn Inode>;
    
    /// 同步文件系统
    ///
    /// 将所有未写入的数据刷新到持久化存储设备
    fn sync(&self) -> Result<(), FsError>;
    
    /// 获取文件系统统计信息
    ///
    /// 返回磁盘使用情况、inode 数量等统计信息
    fn statfs(&self) -> Result<StatFs, FsError>;
    
    /// 卸载文件系统（可选）
    ///
    /// 执行卸载前的清理工作，默认实现调用 sync()
    fn umount(&self) -> Result<(), FsError> {
        self.sync()
    }
}
```

### StatFs 结构

文件系统统计信息：

```rust
#[derive(Debug, Clone)]
pub struct StatFs {
    /// 块大小（单位：字节）
    pub block_size: usize,
    
    /// 总块数
    pub total_blocks: usize,
    
    /// 空闲块数
    pub free_blocks: usize,
    
    /// 可用块数（非特权用户可用）
    pub available_blocks: usize,
    
    /// 总 inode 数
    pub total_inodes: usize,
    
    /// 空闲 inode 数
    pub free_inodes: usize,
    
    /// 文件系统 ID
    pub fsid: u64,
    
    /// 最大文件名长度
    pub max_filename_len: usize,
}
```

## 实现文件系统

### TmpFS 示例

内存文件系统是最简单的文件系统实现：

```rust
pub struct TmpFs {
    root: Arc<TmpfsInode>,
    next_inode_no: AtomicUsize,
}

impl TmpFs {
    pub fn new() -> Self {
        // 创建根目录 inode
        let root = Arc::new(TmpfsInode::new_dir(1));
        
        Self {
            root,
            next_inode_no: AtomicUsize::new(2),
        }
    }
    
    pub fn alloc_inode_no(&self) -> usize {
        self.next_inode_no.fetch_add(1, Ordering::Relaxed)
    }
}

impl FileSystem for TmpFs {
    fn fs_type(&self) -> &'static str {
        "tmpfs"
    }
    
    fn root_inode(&self) -> Arc<dyn Inode> {
        self.root.clone()
    }
    
    fn sync(&self) -> Result<(), FsError> {
        // 内存文件系统无需同步
        Ok(())
    }
    
    fn statfs(&self) -> Result<StatFs, FsError> {
        Ok(StatFs {
            block_size: 4096,
            total_blocks: 0,        // 内存文件系统无限制
            free_blocks: 0,
            available_blocks: 0,
            total_inodes: 0,
            free_inodes: 0,
            fsid: 0,
            max_filename_len: 255,
        })
    }
    
    fn umount(&self) -> Result<(), FsError> {
        // 内存文件系统卸载时释放所有数据
        // 实际上由 Arc 自动管理
        Ok(())
    }
}
```

### Fat32 示例

磁盘文件系统需要管理持久化存储：

```rust
pub struct Fat32Fs {
    device: Arc<dyn BlockDevice>,
    root_inode: Arc<Fat32Inode>,
    fat_table: SpinLock<Vec<u32>>,
    dirty: AtomicBool,
}

impl Fat32Fs {
    pub fn new(device: Arc<dyn BlockDevice>) -> Result<Self, FsError> {
        // 读取引导扇区
        let boot_sector = Self::read_boot_sector(&device)?;
        
        // 加载 FAT 表
        let fat_table = Self::load_fat(&device, &boot_sector)?;
        
        // 创建根目录 inode
        let root_inode = Arc::new(Fat32Inode::new_root(&boot_sector));
        
        Ok(Self {
            device,
            root_inode,
            fat_table: SpinLock::new(fat_table),
            dirty: AtomicBool::new(false),
        })
    }
    
    fn write_fat(&self) -> Result<(), FsError> {
        // 将 FAT 表写回磁盘
        let fat = self.fat_table.lock();
        let buf = fat.as_slice();
        self.device.write(FAT_OFFSET, buf)?;
        Ok(())
    }
}

impl FileSystem for Fat32Fs {
    fn fs_type(&self) -> &'static str {
        "fat32"
    }
    
    fn root_inode(&self) -> Arc<dyn Inode> {
        self.root_inode.clone()
    }
    
    fn sync(&self) -> Result<(), FsError> {
        if self.dirty.load(Ordering::Relaxed) {
            // 写回 FAT 表
            self.write_fat()?;
            
            // 同步所有脏 inode
            // ...
            
            self.dirty.store(false, Ordering::Relaxed);
        }
        Ok(())
    }
    
    fn statfs(&self) -> Result<StatFs, FsError> {
        let total_clusters = self.boot_sector.total_clusters();
        let free_clusters = self.count_free_clusters();
        
        Ok(StatFs {
            block_size: self.boot_sector.cluster_size(),
            total_blocks: total_clusters,
            free_blocks: free_clusters,
            available_blocks: free_clusters,
            total_inodes: 0,  // FAT32 无 inode 限制
            free_inodes: 0,
            fsid: 0,
            max_filename_len: 255,
        })
    }
    
    fn umount(&self) -> Result<(), FsError> {
        // 卸载前同步
        self.sync()?;
        
        // 释放缓存
        // ...
        
        Ok(())
    }
}
```

## 错误处理

### FsError 枚举

VFS 定义了与 POSIX 兼容的错误类型：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    // 文件/目录相关
    NotFound,          // -ENOENT(2): 文件不存在
    AlreadyExists,     // -EEXIST(17): 文件已存在（O_CREAT | O_EXCL）
    NotDirectory,      // -ENOTDIR(20): 不是目录
    IsDirectory,       // -EISDIR(21): 是目录（不能对目录执行文件操作）
    DirectoryNotEmpty, // -ENOTEMPTY(39): 目录非空（rmdir）
    
    // 权限相关
    PermissionDenied,  // -EACCES(13): 权限被拒绝
    
    // 文件描述符相关
    BadFileDescriptor, // -EBADF(9): 无效的文件描述符
    TooManyOpenFiles,  // -EMFILE(24): 进程打开的文件过多
    
    // 参数相关
    InvalidArgument,   // -EINVAL(22): 无效参数
    NameTooLong,       // -ENAMETOOLONG(36): 文件名过长
    
    // 文件系统相关
    ReadOnlyFs,        // -EROFS(30): 只读文件系统
    NoSpace,           // -ENOSPC(28): 设备空间不足
    IoError,           // -EIO(5): I/O 错误
    NoDevice,          // -ENODEV(19): 设备不存在
    
    // 管道相关
    BrokenPipe,        // -EPIPE(32): 管道破裂（读端已关闭）
    WouldBlock,        // -EAGAIN(11): 非阻塞操作将阻塞
    
    // 其他
    NotSupported,      // -ENOTSUP(95): 操作不支持
    TooManyLinks,      // -EMLINK(31): 硬链接过多
}
```

### 错误码转换

FsError 可以转换为系统调用错误码（负数）：

```rust
impl FsError {
    pub fn to_errno(&self) -> isize {
        match self {
            FsError::NotFound => -2,
            FsError::IoError => -5,
            FsError::BadFileDescriptor => -9,
            FsError::WouldBlock => -11,
            FsError::PermissionDenied => -13,
            FsError::AlreadyExists => -17,
            FsError::NoDevice => -19,
            FsError::NotDirectory => -20,
            FsError::IsDirectory => -21,
            FsError::InvalidArgument => -22,
            FsError::TooManyOpenFiles => -24,
            FsError::NoSpace => -28,
            FsError::ReadOnlyFs => -30,
            FsError::TooManyLinks => -31,
            FsError::BrokenPipe => -32,
            FsError::NameTooLong => -36,
            FsError::DirectoryNotEmpty => -39,
            FsError::NotSupported => -95,
        }
    }
}
```

### 错误使用场景

#### NotFound

**使用场景**: 文件或目录不存在

```rust
// 打开不存在的文件（没有 O_CREAT）
let dentry = vfs_lookup("/nonexistent")?;  // Err(NotFound)

// 查找目录中不存在的子项
parent.inode.lookup("missing")?;  // Err(NotFound)
```

#### AlreadyExists

**使用场景**: 文件已存在（O_CREAT | O_EXCL）

```rust
// 创建已存在的文件
if flags.contains(OpenFlags::O_CREAT | OpenFlags::O_EXCL) {
    if file_exists {
        return Err(FsError::AlreadyExists);
    }
}
```

#### PermissionDenied

**使用场景**: 没有足够的权限

```rust
// 尝试写入只读文件
if !file.writable() {
    return Err(FsError::PermissionDenied);
}

// 尝试访问没有权限的文件
if !metadata.mode.can_read() {
    return Err(FsError::PermissionDenied);
}
```

#### IsDirectory / NotDirectory

**使用场景**: 文件类型不匹配

```rust
// 对目录执行文件操作
let metadata = dentry.inode.metadata()?;
if metadata.inode_type == InodeType::Directory {
    return Err(FsError::IsDirectory);
}

// 对文件执行目录操作
if metadata.inode_type != InodeType::Directory {
    return Err(FsError::NotDirectory);
}
```

#### NoSpace

**使用场景**: 磁盘空间不足

```rust
// 写入数据时磁盘满
if self.free_blocks() == 0 {
    return Err(FsError::NoSpace);
}
```

#### WouldBlock

**使用场景**: 非阻塞操作将阻塞

```rust
// 非阻塞管道写入时缓冲区满
if flags.contains(OpenFlags::O_NONBLOCK) && buffer_full() {
    return Err(FsError::WouldBlock);
}
```

### 错误处理最佳实践

#### 1. 总是检查错误

```rust
// 错误❌
let dentry = vfs_lookup(path).unwrap();

// 正确✅
let dentry = vfs_lookup(path)
    .map_err(|e| format!("Failed to lookup {}: {:?}", path, e))?;
```

#### 2. 提供上下文信息

```rust
pub fn open_file(path: &str) -> Result<Arc<RegFile>, String> {
    let dentry = vfs_lookup(path)
        .map_err(|e| format!("open_file: lookup failed for '{}': {:?}", path, e))?;
    
    let metadata = dentry.inode.metadata()
        .map_err(|e| format!("open_file: metadata failed: {:?}", e))?;
    
    if metadata.inode_type == InodeType::Directory {
        return Err(format!("open_file: '{}' is a directory", path));
    }
    
    Ok(Arc::new(RegFile::new(dentry, OpenFlags::O_RDONLY)))
}
```

#### 3. 区分预期错误和异常错误

```rust
pub fn try_create_file(path: &str) -> Result<Arc<RegFile>, FsError> {
    match vfs_lookup(path) {
        Ok(dentry) => {
            // 文件已存在，正常情况
            Ok(Arc::new(RegFile::new(dentry, OpenFlags::O_RDWR)))
        }
        Err(FsError::NotFound) => {
            // 文件不存在，创建新文件（预期行为）
            let (dir, name) = split_path(path)?;
            let parent = vfs_lookup(&dir)?;
            let inode = parent.inode.create(&name, 
                FileMode::S_IFREG | FileMode::S_IRUSR | FileMode::S_IWUSR)?;
            let dentry = Dentry::new(name, inode);
            Ok(Arc::new(RegFile::new(dentry, OpenFlags::O_RDWR)))
        }
        Err(e) => {
            // 其他错误，异常情况
            Err(e)
        }
    }
}
```

## 使用示例

### 挂载自定义文件系统

```rust
// 创建文件系统实例
let my_fs = Arc::new(MyCustomFs::new());

// 挂载到 /mnt
vfs::MOUNT_TABLE.mount(
    my_fs,
    "/mnt",
    MountFlags::empty(),
    Some(String::from("/dev/custom"))
)?;

// 访问挂载的文件系统
let dentry = vfs::vfs_lookup("/mnt/file.txt")?;
```

### 查询文件系统统计信息

```rust
pub fn sys_statfs(path: &str) -> Result<StatFs, FsError> {
    let dentry = vfs_lookup(path)?;
    let full_path = dentry.full_path();
    
    // 查找挂载点
    let mount_point = vfs::MOUNT_TABLE.find_mount(&full_path)
        .ok_or(FsError::NotSupported)?;
    
    // 获取统计信息
    mount_point.fs.statfs()
}
```

### 同步文件系统

```rust
pub fn sys_sync() {
    // 同步所有挂载的文件系统
    let mounts = vfs::MOUNT_TABLE.list_all();
    for (_, mount_point) in mounts {
        let _ = mount_point.fs.sync();
    }
}
```

## 常见问题

### Q: FileSystem 和 Inode 有什么区别？

A:
- **FileSystem**: 文件系统级别的操作（根 inode、同步、统计）
- **Inode**: 单个文件/目录的操作（读写、查找、创建）

### Q: 为什么 tmpfs 的 sync() 是空操作？

A:
tmpfs 是内存文件系统，数据只存在内存中，没有持久化存储，因此无需同步。

### Q: 如何实现自己的文件系统？

A:
1. 实现 `Inode` trait 定义单个文件/目录的行为
2. 实现 `FileSystem` trait 提供文件系统级别的接口
3. 在 `sys_mount` 中添加对新文件系统类型的支持

### Q: 错误处理中如何选择合适的错误类型？

A:
参考 POSIX 标准的 errno 定义，选择最接近的错误类型。如果没有合适的，使用 `IoError` 或 `NotSupported`。

### Q: umount 失败会怎样？

A:
卸载失败通常是因为：
- 文件系统正在使用（有打开的文件）
- sync() 失败（磁盘错误）

卸载失败后挂载点仍然保留，可以稍后重试。

## 相关资源

### 源代码位置

- **FileSystem trait**: `os/src/vfs/file_system.rs`
- **FsError**: `os/src/vfs/error.rs`
- **tmpfs 实现**: `os/src/fs/tmpfs/`
- **fat32 实现**: `os/src/fs/fat32/`

### 参考文档

- [VFS 整体架构](architecture.md)
- [Inode 与 Dentry](inode_and_dentry.md)
- [路径解析与挂载](path_and_mount.md)
- [使用指南](usage.md)

### POSIX 错误码参考

| errno | 值 | VFS 对应 | 说明 |
|-------|---|----------|------|
| ENOENT | 2 | NotFound | 文件不存在 |
| EIO | 5 | IoError | I/O 错误 |
| EBADF | 9 | BadFileDescriptor | 无效文件描述符 |
| EAGAIN | 11 | WouldBlock | 资源暂时不可用 |
| EACCES | 13 | PermissionDenied | 权限被拒绝 |
| EEXIST | 17 | AlreadyExists | 文件已存在 |
| ENOTDIR | 20 | NotDirectory | 不是目录 |
| EISDIR | 21 | IsDirectory | 是目录 |
| EINVAL | 22 | InvalidArgument | 无效参数 |
| EMFILE | 24 | TooManyOpenFiles | 打开文件过多 |
| ENOSPC | 28 | NoSpace | 设备空间不足 |
| EROFS | 30 | ReadOnlyFs | 只读文件系统 |
