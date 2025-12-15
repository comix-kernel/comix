# File 与 FDTable

## 概述

本文档详细介绍 VFS 子系统的会话层 (File trait) 和文件描述符表 (FDTable) 的设计与实现。File trait 定义了统一的文件操作接口，支持多种文件类型；FDTable 管理进程级的文件描述符空间。

## File Trait - 会话层接口

### 核心概念

File trait 是 VFS 会话层的核心抽象，定义了有状态的文件操作接口。与存储层的 Inode trait 不同，File 方法不携带 offset 参数，而是在内部维护当前读写位置。

#### File 与 Inode 的区别

| 方面 | File (会话层) | Inode (存储层) |
|------|---------------|----------------|
| 状态 | 有状态 (维护 offset、flags) | 无状态 |
| 方法签名 | `read(buf)` | `read_at(offset, buf)` |
| 实例数量 | 每次 open 创建新实例 | 多个 File 可共享同一 Inode |
| 存储位置 | FDTable 中 | Dentry 中 |
| 生命周期 | 随文件描述符关闭而结束 | 随 Dentry 释放而结束 |

### File Trait 定义

```rust
pub trait File: Send + Sync {
    // 基本属性查询
    fn readable(&self) -> bool;
    fn writable(&self) -> bool;
    
    // 核心 I/O 操作
    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError>;
    fn write(&self, buf: &[u8]) -> Result<usize, FsError>;
    fn metadata(&self) -> Result<InodeMetadata, FsError>;
    
    // 可选方法 (默认返回 NotSupported)
    fn lseek(&self, offset: isize, whence: SeekWhence) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }
    
    fn offset(&self) -> usize { 0 }
    fn flags(&self) -> OpenFlags { OpenFlags::empty() }
    
    fn dentry(&self) -> Result<Arc<Dentry>, FsError> {
        Err(FsError::NotSupported)
    }
    
    fn inode(&self) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotSupported)
    }
    
    // 高级操作
    fn set_status_flags(&self, flags: OpenFlags) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }
    
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }
    
    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }
    
    // 管道特定操作
    fn get_pipe_size(&self) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }
    
    fn set_pipe_size(&self, size: usize) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }
    
    // 异步 I/O
    fn get_owner(&self) -> Result<i32, FsError> {
        Err(FsError::NotSupported)
    }
    
    fn set_owner(&self, pid: i32) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }
    
    // 设备控制
    fn ioctl(&self, request: u32, arg: usize) -> Result<isize, FsError> {
        Err(FsError::NotSupported)
    }
}
```

## 文件类型实现

### RegFile - 普通文件

RegFile 是基于 Inode 的普通文件实现，支持 seek 操作。

#### RegFile 结构

```rust
pub struct RegFile {
    dentry: Arc<Dentry>,
    offset: AtomicUsize,
    flags: OpenFlags,
}

impl RegFile {
    pub fn new(dentry: Arc<Dentry>, flags: OpenFlags) -> Self {
        Self {
            dentry,
            offset: AtomicUsize::new(0),
            flags,
        }
    }
}
```

#### RegFile 实现要点

```rust
impl File for RegFile {
    fn readable(&self) -> bool {
        let mode = self.flags & OpenFlags::O_ACCMODE;
        mode == OpenFlags::O_RDONLY || mode == OpenFlags::O_RDWR
    }
    
    fn writable(&self) -> bool {
        let mode = self.flags & OpenFlags::O_ACCMODE;
        mode == OpenFlags::O_WRONLY || mode == OpenFlags::O_RDWR
    }
    
    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        if !self.readable() {
            return Err(FsError::PermissionDenied);
        }
        
        let offset = self.offset.load(Ordering::Relaxed);
        let n = self.dentry.inode.read_at(offset, buf)?;
        self.offset.fetch_add(n, Ordering::Relaxed);
        Ok(n)
    }
    
    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        if !self.writable() {
            return Err(FsError::PermissionDenied);
        }
        
        let offset = if self.flags.contains(OpenFlags::O_APPEND) {
            // 追加模式：总是写到文件末尾
            self.dentry.inode.metadata()?.size
        } else {
            self.offset.load(Ordering::Relaxed)
        };
        
        let n = self.dentry.inode.write_at(offset, buf)?;
        
        if !self.flags.contains(OpenFlags::O_APPEND) {
            self.offset.fetch_add(n, Ordering::Relaxed);
        }
        
        Ok(n)
    }
    
    fn lseek(&self, offset: isize, whence: SeekWhence) -> Result<usize, FsError> {
        let new_offset = match whence {
            SeekWhence::SET => offset as usize,
            SeekWhence::CUR => {
                let cur = self.offset.load(Ordering::Relaxed);
                (cur as isize + offset) as usize
            }
            SeekWhence::END => {
                let size = self.dentry.inode.metadata()?.size;
                (size as isize + offset) as usize
            }
        };
        
        self.offset.store(new_offset, Ordering::Relaxed);
        Ok(new_offset)
    }
    
    fn offset(&self) -> usize {
        self.offset.load(Ordering::Relaxed)
    }
    
    fn flags(&self) -> OpenFlags {
        self.flags
    }
    
    fn dentry(&self) -> Result<Arc<Dentry>, FsError> {
        Ok(self.dentry.clone())
    }
    
    fn inode(&self) -> Result<Arc<dyn Inode>, FsError> {
        Ok(self.dentry.inode.clone())
    }
    
    // 支持 pread/pwrite (不改变 offset)
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        self.dentry.inode.read_at(offset, buf)
    }
    
    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        self.dentry.inode.write_at(offset, buf)
    }
}
```

### PipeFile - 管道文件

PipeFile 是流式设备，不支持 seek，使用环形缓冲区实现。

#### PipeFile 结构

```rust
pub struct PipeFile {
    pipe: Arc<Pipe>,
    mode: PipeMode,
}

pub enum PipeMode {
    Read,
    Write,
}

struct Pipe {
    buffer: SpinLock<VecDeque<u8>>,
    capacity: usize,
    read_closed: AtomicBool,
    write_closed: AtomicBool,
}
```

#### PipeFile 实现要点

```rust
impl File for PipeFile {
    fn readable(&self) -> bool {
        matches!(self.mode, PipeMode::Read)
    }
    
    fn writable(&self) -> bool {
        matches!(self.mode, PipeMode::Write)
    }
    
    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        if !self.readable() {
            return Err(FsError::PermissionDenied);
        }
        
        let mut buffer = self.pipe.buffer.lock();
        
        // 如果缓冲区为空且写端已关闭，返回 EOF
        if buffer.is_empty() && self.pipe.write_closed.load(Ordering::Relaxed) {
            return Ok(0);
        }
        
        // 从缓冲区读取数据
        let len = core::cmp::min(buf.len(), buffer.len());
        for i in 0..len {
            buf[i] = buffer.pop_front().unwrap();
        }
        
        Ok(len)
    }
    
    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        if !self.writable() {
            return Err(FsError::PermissionDenied);
        }
        
        if self.pipe.read_closed.load(Ordering::Relaxed) {
            return Err(FsError::BrokenPipe);
        }
        
        let mut buffer = self.pipe.buffer.lock();
        
        // 检查容量
        if buffer.len() + buf.len() > self.pipe.capacity {
            return Err(FsError::WouldBlock);
        }
        
        for &byte in buf {
            buffer.push_back(byte);
        }
        
        Ok(buf.len())
    }
    
    fn get_pipe_size(&self) -> Result<usize, FsError> {
        Ok(self.pipe.capacity)
    }
    
    fn set_pipe_size(&self, size: usize) -> Result<(), FsError> {
        // 简化实现，实际需要检查 MIN_PIPE_SIZE 和 MAX_PIPE_SIZE
        self.pipe.capacity = size;
        Ok(())
    }
    
    // 管道不支持 seek
    fn lseek(&self, _offset: isize, _whence: SeekWhence) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }
}
```

### StdioFile - 标准 I/O 文件

StdioFile 包装控制台输入输出，提供统一的 File 接口。

#### StdioFile 实现

```rust
pub struct StdinFile;
pub struct StdoutFile;
pub struct StderrFile;

impl File for StdinFile {
    fn readable(&self) -> bool { true }
    fn writable(&self) -> bool { false }
    
    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        // 从控制台读取（阻塞）
        console::stdin().read(buf).map_err(|_| FsError::IoError)
    }
    
    fn write(&self, _buf: &[u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied)
    }
    
    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        Ok(InodeMetadata {
            inode_type: InodeType::CharDevice,
            mode: FileMode::S_IFCHR | FileMode::S_IRUSR,
            ..Default::default()
        })
    }
}

impl File for StdoutFile {
    fn readable(&self) -> bool { false }
    fn writable(&self) -> bool { true }
    
    fn read(&self, _buf: &mut [u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied)
    }
    
    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        console::stdout().write(buf).map_err(|_| FsError::IoError)
    }
    
    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        Ok(InodeMetadata {
            inode_type: InodeType::CharDevice,
            mode: FileMode::S_IFCHR | FileMode::S_IWUSR,
            ..Default::default()
        })
    }
}

// StderrFile 与 StdoutFile 类似
```

### CharDevFile - 字符设备文件

字符设备文件通过设备驱动提供 I/O 功能。

```rust
pub struct CharDevFile {
    dev: u64,
    flags: OpenFlags,
}

impl File for CharDevFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        let driver = get_chrdev_driver(major(self.dev))?;
        driver.read(minor(self.dev), buf)
    }
    
    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        let driver = get_chrdev_driver(major(self.dev))?;
        driver.write(minor(self.dev), buf)
    }
    
    fn ioctl(&self, request: u32, arg: usize) -> Result<isize, FsError> {
        let driver = get_chrdev_driver(major(self.dev))?;
        driver.ioctl(minor(self.dev), request, arg)
    }
}
```

### BlkDevFile - 块设备文件

块设备文件支持随机访问，通常用于磁盘等存储设备。

```rust
pub struct BlkDevFile {
    dev: u64,
    offset: AtomicUsize,
    flags: OpenFlags,
}

impl File for BlkDevFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        let offset = self.offset.load(Ordering::Relaxed);
        let driver = get_blkdev_driver(major(self.dev))?;
        let n = driver.read_at(minor(self.dev), offset, buf)?;
        self.offset.fetch_add(n, Ordering::Relaxed);
        Ok(n)
    }
    
    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        let offset = self.offset.load(Ordering::Relaxed);
        let driver = get_blkdev_driver(major(self.dev))?;
        let n = driver.write_at(minor(self.dev), offset, buf)?;
        self.offset.fetch_add(n, Ordering::Relaxed);
        Ok(n)
    }
    
    fn lseek(&self, offset: isize, whence: SeekWhence) -> Result<usize, FsError> {
        // 块设备支持 seek
        let new_offset = match whence {
            SeekWhence::SET => offset as usize,
            SeekWhence::CUR => {
                let cur = self.offset.load(Ordering::Relaxed);
                (cur as isize + offset) as usize
            }
            SeekWhence::END => {
                let size = self.metadata()?.size;
                (size as isize + offset) as usize
            }
        };
        self.offset.store(new_offset, Ordering::Relaxed);
        Ok(new_offset)
    }
}
```

## FDTable - 文件描述符表

### 核心概念

FDTable (File Descriptor Table) 是进程级资源，管理打开的文件。每个进程有独立的 FDTable，文件描述符是进程特定的整数索引。

#### FDTable 的职责

- **分配文件描述符**: 总是返回最小可用的 FD (POSIX 要求)
- **文件生命周期管理**: 通过 Arc 引用计数自动释放文件
- **dup 语义**: 支持文件描述符复制，共享 File 对象
- **close-on-exec**: 管理 FD_CLOEXEC 标志

### FDTable 结构

```rust
pub struct FDTable {
    /// 文件描述符数组
    files: SpinLock<Vec<Option<Arc<dyn File>>>>,
    
    /// FD 标志数组 (与 files 索引对应)
    fd_flags: SpinLock<Vec<FdFlags>>,
    
    /// 最大文件描述符数量
    max_fds: usize,
}

bitflags! {
    pub struct FdFlags: u32 {
        const CLOEXEC = 1;  // Close on exec
    }
}
```

### FDTable 方法

#### 分配文件描述符

```rust
impl FDTable {
    pub fn alloc(&self, file: Arc<dyn File>) -> Result<usize, FsError> {
        self.alloc_with_flags(file, FdFlags::empty())
    }
    
    pub fn alloc_with_flags(&self, file: Arc<dyn File>, flags: FdFlags) 
        -> Result<usize, FsError> {
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();
        
        // 查找最小可用 FD
        for (fd, slot) in files.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(file);
                fd_flags[fd] = flags;
                return Ok(fd);
            }
        }
        
        // 扩展数组
        let fd = files.len();
        if fd >= self.max_fds {
            return Err(FsError::TooManyOpenFiles);
        }
        
        files.push(Some(file));
        fd_flags.push(flags);
        Ok(fd)
    }
    
    pub fn install_at(&self, fd: usize, file: Arc<dyn File>) 
        -> Result<(), FsError> {
        self.install_at_with_flags(fd, file, FdFlags::empty())
    }
    
    pub fn install_at_with_flags(&self, fd: usize, file: Arc<dyn File>, 
                                 flags: FdFlags) -> Result<(), FsError> {
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();
        
        if fd >= self.max_fds {
            return Err(FsError::InvalidArgument);
        }
        
        // 扩展数组到指定大小
        while files.len() <= fd {
            files.push(None);
            fd_flags.push(FdFlags::empty());
        }
        
        files[fd] = Some(file);
        fd_flags[fd] = flags;
        Ok(())
    }
}
```

#### 访问和关闭

```rust
impl FDTable {
    pub fn get(&self, fd: usize) -> Result<Arc<dyn File>, FsError> {
        let files = self.files.lock();
        files.get(fd)
            .and_then(|f| f.clone())
            .ok_or(FsError::BadFileDescriptor)
    }
    
    pub fn close(&self, fd: usize) -> Result<(), FsError> {
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();
        
        if fd >= files.len() || files[fd].is_none() {
            return Err(FsError::BadFileDescriptor);
        }
        
        files[fd] = None;
        fd_flags[fd] = FdFlags::empty();
        Ok(())
    }
}
```

#### dup 系列操作

```rust
impl FDTable {
    /// dup: 复制文件描述符
    pub fn dup(&self, old_fd: usize) -> Result<usize, FsError> {
        let file = self.get(old_fd)?;
        self.alloc(file)
    }
    
    /// dup2: 复制到指定 FD
    pub fn dup2(&self, old_fd: usize, new_fd: usize) -> Result<usize, FsError> {
        // 特殊情况: old_fd == new_fd
        if old_fd == new_fd {
            self.get(old_fd)?;  // 检查有效性
            return Ok(new_fd);
        }
        
        let file = self.get(old_fd)?;
        let _ = self.close(new_fd);  // 忽略错误
        self.install_at(new_fd, file)?;
        Ok(new_fd)
    }
    
    /// dup3: dup2 + 支持设置标志
    pub fn dup3(&self, old_fd: usize, new_fd: usize, flags: OpenFlags) 
        -> Result<usize, FsError> {
        // dup3 不允许 old_fd == new_fd
        if old_fd == new_fd {
            return Err(FsError::InvalidArgument);
        }
        
        let file = self.get(old_fd)?;
        let _ = self.close(new_fd);
        
        let fd_flags = FdFlags::from_open_flags(flags);
        self.install_at_with_flags(new_fd, file, fd_flags)?;
        Ok(new_fd)
    }
}
```

#### fork 和 exec 支持

```rust
impl FDTable {
    /// 克隆整个表 (用于 fork)
    pub fn clone_table(&self) -> Self {
        let files = self.files.lock().clone();
        let fd_flags = self.fd_flags.lock().clone();
        Self {
            files: SpinLock::new(files),
            fd_flags: SpinLock::new(fd_flags),
            max_fds: self.max_fds,
        }
    }
    
    /// 关闭带 CLOEXEC 标志的文件 (用于 exec)
    pub fn close_exec(&self) {
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();
        
        for (slot, flags) in files.iter_mut().zip(fd_flags.iter_mut()) {
            if flags.contains(FdFlags::CLOEXEC) {
                *slot = None;
                *flags = FdFlags::empty();
            }
        }
    }
}
```

#### FD 标志管理

```rust
impl FDTable {
    pub fn get_fd_flags(&self, fd: usize) -> Result<FdFlags, FsError> {
        let files = self.files.lock();
        let fd_flags = self.fd_flags.lock();
        
        if fd >= files.len() || files[fd].is_none() {
            return Err(FsError::BadFileDescriptor);
        }
        
        Ok(fd_flags[fd])
    }
    
    pub fn set_fd_flags(&self, fd: usize, flags: FdFlags) -> Result<(), FsError> {
        let files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();
        
        if fd >= files.len() || files[fd].is_none() {
            return Err(FsError::BadFileDescriptor);
        }
        
        fd_flags[fd] = flags;
        Ok(())
    }
}
```

## 使用示例

### 打开和读取文件

```rust
// 1. 打开文件
let dentry = vfs_lookup("/etc/passwd")?;
let file = Arc::new(RegFile::new(dentry, OpenFlags::O_RDONLY));

// 2. 安装到 FDTable
let fd_table = current_task().lock().fd_table.clone();
let fd = fd_table.alloc(file)?;

// 3. 读取数据
let file = fd_table.get(fd)?;
let mut buf = [0u8; 1024];
let n = file.read(&mut buf)?;

// 4. 关闭文件
fd_table.close(fd)?;
```

### 创建管道

```rust
pub fn create_pipe() -> Result<(Arc<PipeFile>, Arc<PipeFile>), FsError> {
    let pipe = Arc::new(Pipe::new(4096));  // 4KB 缓冲区
    
    let read_end = Arc::new(PipeFile {
        pipe: pipe.clone(),
        mode: PipeMode::Read,
    });
    
    let write_end = Arc::new(PipeFile {
        pipe: pipe.clone(),
        mode: PipeMode::Write,
    });
    
    Ok((read_end, write_end))
}

// 使用管道
let (read_file, write_file) = create_pipe()?;
let read_fd = fd_table.alloc(read_file)?;
let write_fd = fd_table.alloc(write_file)?;

// 写入数据
let file = fd_table.get(write_fd)?;
file.write(b"Hello, pipe!")?;

// 读取数据
let file = fd_table.get(read_fd)?;
let mut buf = [0u8; 128];
let n = file.read(&mut buf)?;
```

### dup 重定向

```rust
// 将 stdout 重定向到文件
let dentry = vfs_lookup("/tmp/output.txt")?;
let file = Arc::new(RegFile::new(dentry, 
    OpenFlags::O_WRONLY | OpenFlags::O_CREAT | OpenFlags::O_TRUNC));

let fd = fd_table.alloc(file)?;
fd_table.dup2(fd, 1)?;  // 1 = stdout
fd_table.close(fd)?;

// 现在 println! 会写到文件
```

## 最佳实践

### 实现 File 时的注意事项

1. **线程安全**: File 必须实现 `Send + Sync`，内部状态需要原子操作或锁保护
2. **权限检查**: read/write 前检查 readable()/writable()
3. **错误处理**: 返回准确的 FsError (PermissionDenied/WouldBlock 等)
4. **可选方法**: 不支持的方法返回 `Err(FsError::NotSupported)`

### 使用 FDTable 时的注意事项

1. **及时关闭**: 避免文件描述符泄漏，使用 RAII 模式管理
2. **检查返回值**: get/close 可能返回错误，必须处理
3. **dup 语义**: dup 后的 FD 共享 offset，注意并发访问
4. **fork 后**: 父子进程共享 FDTable，修改会相互影响

### 性能优化建议

1. **批量 I/O**: 使用较大的缓冲区，减少系统调用次数
2. **避免 lseek**: 顺序读写不需要 lseek，直接 read/write
3. **管道大小**: 根据使用场景调整管道缓冲区大小
4. **异步 I/O**: 对于网络文件系统，考虑异步实现

## 常见问题

### Q: File 和 Inode 都有 read 方法，有什么区别?

A: 
- **File::read(buf)**: 从当前 offset 读取，自动更新 offset
- **Inode::read_at(offset, buf)**: 从指定 offset 读取，不改变状态
- 一个 Inode 可以被多个 File 共享，各自维护独立的 offset

### Q: dup 后的文件描述符共享什么?

A: 
- **共享**: File 对象 (包括 offset)，文件状态标志 (O_APPEND 等)
- **不共享**: FD 标志 (FD_CLOEXEC)

### Q: O_CLOEXEC 和 FD_CLOEXEC 有什么区别?

A: 
- **O_CLOEXEC**: open() 时指定，自动设置 FD_CLOEXEC 标志
- **FD_CLOEXEC**: FD 标志，通过 fcntl(F_SETFD) 设置

### Q: 管道缓冲区满了怎么办?

A: 
当前实现返回 `WouldBlock` 错误。完整实现应该:
- 如果是阻塞模式，阻塞等待缓冲区有空间
- 如果是非阻塞模式 (O_NONBLOCK)，返回 WouldBlock

### Q: 如何实现 O_NONBLOCK?

A: 
File 实现需要检查 flags，在 read/write 时:
- 阻塞模式: 等待数据/空间可用
- 非阻塞模式: 立即返回 WouldBlock

## 相关资源

### 源代码位置

- **File trait**: `os/src/vfs/file.rs`
- **FDTable**: `os/src/vfs/fd_table.rs`
- **RegFile**: `os/src/vfs/impls/reg_file.rs`
- **PipeFile**: `os/src/vfs/impls/pipe_file.rs`
- **StdioFile**: `os/src/vfs/impls/stdio_file.rs`
- **设备文件**: `os/src/vfs/impls/char_dev_file.rs`, `os/src/vfs/impls/blk_dev_file.rs`

### 参考文档

- [VFS 整体架构](architecture.md)
- [Inode 与 Dentry](inode_and_dentry.md)
- [路径解析与挂载](path_and_mount.md)
- [使用指南](usage.md)
