# 文件锁与设备管理

## 概述

本文档介绍 VFS 的文件锁机制和设备管理功能。文件锁实现 POSIX advisory locks 语义，支持进程间文件访问同步；设备管理提供字符设备和块设备的统一抽象。

## 文件锁机制

### 核心概念

文件锁（File Locks）是进程间同步文件访问的机制。VFS 实现 POSIX advisory locks，即建议性锁，不强制执行，需要进程协作遵守。

#### 锁类型

```rust
pub enum LockType {
    Read = 0,    // F_RDLCK: 读锁（共享锁）
    Write = 1,   // F_WRLCK: 写锁（独占锁）
    Unlock = 2,  // F_UNLCK: 解锁
}
```

#### 锁的语义

| 已持有 \ 请求 | 读锁 (共享) | 写锁 (独占) |
|--------------|------------|------------|
| **无锁** | ✅ 允许 | ✅ 允许 |
| **读锁** | ✅ 允许（可共享） | ❌ 冲突 |
| **写锁** | ❌ 冲突 | ❌ 冲突 |

**特殊规则**:
- 同一进程的锁不冲突（可以升级/降级锁）
- 进程退出时自动释放所有锁

### FileLockEntry 结构

单个锁的表示：

```rust
struct FileLockEntry {
    /// 锁类型（读/写）
    lock_type: LockType,
    
    /// 起始位置（文件中的绝对偏移）
    start: usize,
    
    /// 长度（0 表示锁定到文件末尾）
    len: usize,
    
    /// 持有锁的进程 PID
    pid: i32,
}
```

### FileLockManager

全局文件锁管理器：

```rust
pub struct FileLockManager {
    /// 文件锁表：FileId -> 锁列表
    locks: SpinLock<BTreeMap<FileId, Vec<FileLockEntry>>>,
}

// 文件标识符
struct FileId {
    dev: u64,   // 设备号
    ino: u64,   // Inode 号
}
```

### fcntl 文件锁操作

#### F_GETLK - 测试锁

检查是否有锁会阻塞请求的锁：

```rust
pub fn test_lock(
    &self,
    dev: u64,
    ino: u64,
    start: usize,
    len: usize,
    flock: &mut Flock,
    pid: i32,
) -> Result<(), FsError> {
    let file_id = FileId { dev, ino };
    let locks = self.locks.lock();
    
    // 构造请求的锁
    let requested_lock = FileLockEntry {
        lock_type: LockType::from_raw(flock.l_type).ok_or(FsError::InvalidArgument)?,
        start,
        len,
        pid,
    };
    
    // 检查是否有冲突的锁
    if let Some(file_locks) = locks.get(&file_id) {
        for existing_lock in file_locks {
            if existing_lock.conflicts_with(&requested_lock) {
                // 找到冲突的锁，填充 flock 结构
                flock.l_type = existing_lock.lock_type as i16;
                flock.l_start = existing_lock.start as i64;
                flock.l_len = existing_lock.len as i64;
                flock.l_pid = existing_lock.pid;
                return Ok(());
            }
        }
    }
    
    // 没有冲突，设置为 F_UNLCK
    flock.l_type = LockType::Unlock as i16;
    Ok(())
}
```

#### F_SETLK / F_SETLKW - 设置锁

```rust
pub fn set_lock(
    &self,
    dev: u64,
    ino: u64,
    start: usize,
    len: usize,
    lock_type: LockType,
    pid: i32,
    blocking: bool,  // true = F_SETLKW, false = F_SETLK
) -> Result<(), FsError> {
    let file_id = FileId { dev, ino };
    let mut locks = self.locks.lock();
    
    match lock_type {
        LockType::Unlock => {
            // 释放锁
            if let Some(file_locks) = locks.get_mut(&file_id) {
                file_locks.retain(|lock| 
                    !(lock.pid == pid && lock.overlaps(start, len))
                );
                if file_locks.is_empty() {
                    locks.remove(&file_id);
                }
            }
            Ok(())
        }
        LockType::Read | LockType::Write => {
            let file_locks = locks.entry(file_id).or_insert_with(Vec::new);
            
            let new_lock = FileLockEntry {
                lock_type,
                start,
                len,
                pid,
            };
            
            // 检查冲突
            for existing_lock in file_locks.iter() {
                if existing_lock.conflicts_with(&new_lock) {
                    if blocking {
                        // TODO: 阻塞等待
                        // 当前实现未完成 F_SETLKW
                        return Err(FsError::WouldBlock);
                    } else {
                        return Err(FsError::WouldBlock);
                    }
                }
            }
            
            // 移除同一进程的旧锁
            file_locks.retain(|lock| 
                !(lock.pid == pid && lock.overlaps(start, len))
            );
            
            // 添加新锁
            file_locks.push(new_lock);
            Ok(())
        }
    }
}
```

#### release_all_locks - 进程退出清理

```rust
pub fn release_all_locks(&self, pid: i32) {
    let mut locks = self.locks.lock();
    for file_locks in locks.values_mut() {
        file_locks.retain(|lock| lock.pid != pid);
    }
    locks.retain(|_, file_locks| !file_locks.is_empty());
}
```

### 使用示例

#### 获取读锁

```rust
use vfs::file_lock_manager;

pub fn acquire_read_lock(file: &Arc<dyn File>) -> Result<(), FsError> {
    let dentry = file.dentry()?;
    let metadata = dentry.inode.metadata()?;
    
    let current = current_task();
    let pid = current.lock().pid;
    
    file_lock_manager().set_lock(
        0,  // dev (简化, 实际需要从 metadata 获取)
        metadata.inode_no as u64,
        0,      // start: 从文件开头
        0,      // len: 0 表示到文件末尾
        LockType::Read,
        pid,
        false,  // 非阻塞
    )
}
```

#### 升级为写锁

```rust
pub fn upgrade_to_write_lock(file: &Arc<dyn File>) -> Result<(), FsError> {
    let dentry = file.dentry()?;
    let metadata = dentry.inode.metadata()?;
    
    let current = current_task();
    let pid = current.lock().pid;
    
    // 同一进程可以升级锁
    file_lock_manager().set_lock(
        0,
        metadata.inode_no as u64,
        0,
        0,
        LockType::Write,
        pid,
        true,  // 阻塞等待
    )
}
```

#### 释放锁

```rust
pub fn release_lock(file: &Arc<dyn File>) -> Result<(), FsError> {
    let dentry = file.dentry()?;
    let metadata = dentry.inode.metadata()?;
    
    let current = current_task();
    let pid = current.lock().pid;
    
    file_lock_manager().set_lock(
        0,
        metadata.inode_no as u64,
        0,
        0,
        LockType::Unlock,
        pid,
        false,
    )
}
```

### 限制与注意事项

#### 当前未实现的功能

1. **F_SETLKW 阻塞等待**: 当前遇到锁冲突时立即返回 `WouldBlock`，即使指定了阻塞模式
   - 完整实现需要等待队列和任务调度支持
   - 需要处理信号中断（返回 EINTR）

2. **死锁检测**: 不检测死锁情况
   - 可能导致多个进程相互等待

3. **锁的范围合并**: 不自动合并相邻的锁
   - 可能导致锁表膨胀

#### Advisory Locks 注意事项

- **建议性**: 锁不是强制的，进程可以忽略锁直接读写
- **协作**: 需要所有进程都遵守锁协议
- **自动释放**: 进程退出或 exec 时自动释放

## 设备管理

### 核心概念

设备文件是访问硬件设备的接口。VFS 支持两种设备类型：
- **字符设备**: 面向流的设备，如串口、终端
- **块设备**: 面向块的设备，如磁盘

### 设备号

设备号由主设备号和次设备号组成：

```rust
// dev.rs

/// 设备号工具函数

/// 从主设备号和次设备号构造设备号
pub fn makedev(major: u32, minor: u32) -> u64 {
    ((major as u64) << 32) | (minor as u64)
}

/// 提取主设备号
pub fn major(dev: u64) -> u32 {
    (dev >> 32) as u32
}

/// 提取次设备号
pub fn minor(dev: u64) -> u32 {
    (dev & 0xFFFFFFFF) as u32
}
```

**说明**:
- **主设备号**: 标识设备类型/驱动程序（如 1 = 内存设备，8 = SCSI 磁盘）
- **次设备号**: 标识同类型设备的具体实例（如 /dev/sda1, /dev/sda2）

### 设备驱动注册

#### 字符设备驱动

```rust
// devno.rs

pub trait CharDeviceDriver: Send + Sync {
    fn read(&self, minor: u32, buf: &mut [u8]) -> Result<usize, FsError>;
    fn write(&self, minor: u32, buf: &[u8]) -> Result<usize, FsError>;
    fn ioctl(&self, minor: u32, request: u32, arg: usize) 
        -> Result<isize, FsError>;
}

// 全局驱动注册表
static CHRDEV_DRIVERS: SpinLock<BTreeMap<u32, Arc<dyn CharDeviceDriver>>> 
    = SpinLock::new(BTreeMap::new());

/// 注册字符设备驱动
pub fn register_chrdev(major: u32, driver: Arc<dyn CharDeviceDriver>) {
    CHRDEV_DRIVERS.lock().insert(major, driver);
}

/// 获取字符设备驱动
pub fn get_chrdev_driver(major: u32) -> Result<Arc<dyn CharDeviceDriver>, FsError> {
    CHRDEV_DRIVERS.lock()
        .get(&major)
        .cloned()
        .ok_or(FsError::NoDevice)
}
```

#### 块设备驱动

```rust
pub trait BlockDeviceDriver: Send + Sync {
    fn block_size(&self) -> usize;
    fn total_blocks(&self, minor: u32) -> usize;
    
    fn read_block(&self, minor: u32, block_no: usize, buf: &mut [u8]) 
        -> Result<usize, FsError>;
    fn write_block(&self, minor: u32, block_no: usize, buf: &[u8]) 
        -> Result<usize, FsError>;
    
    fn read_at(&self, minor: u32, offset: usize, buf: &mut [u8]) 
        -> Result<usize, FsError>;
    fn write_at(&self, minor: u32, offset: usize, buf: &[u8]) 
        -> Result<usize, FsError>;
}

static BLKDEV_DRIVERS: SpinLock<BTreeMap<u32, Arc<dyn BlockDeviceDriver>>> 
    = SpinLock::new(BTreeMap::new());

pub fn register_blkdev(major: u32, driver: Arc<dyn BlockDeviceDriver>) {
    BLKDEV_DRIVERS.lock().insert(major, driver);
}

pub fn get_blkdev_driver(major: u32) -> Result<Arc<dyn BlockDeviceDriver>, FsError> {
    BLKDEV_DRIVERS.lock()
        .get(&major)
        .cloned()
        .ok_or(FsError::NoDevice)
}
```

### 创建设备文件

#### mknod 系统调用

```rust
pub fn sys_mknod(path: &str, mode: FileMode, dev: u64) 
    -> Result<(), FsError> {
    let (dir, name) = vfs::split_path(path)?;
    let parent = vfs::vfs_lookup(&dir)?;
    
    parent.inode.mknod(&name, mode, dev)?;
    Ok(())
}
```

#### 使用示例

```rust
// 创建字符设备文件 /dev/null (major=1, minor=3)
sys_mknod("/dev/null", 
    FileMode::S_IFCHR | FileMode::S_IRUSR | FileMode::S_IWUSR,
    makedev(1, 3))?;

// 创建块设备文件 /dev/sda1 (major=8, minor=1)
sys_mknod("/dev/sda1",
    FileMode::S_IFBLK | FileMode::S_IRUSR | FileMode::S_IWUSR,
    makedev(8, 1))?;
```

### 实现设备驱动示例

#### Null 设备驱动

```rust
struct NullDevice;

impl CharDeviceDriver for NullDevice {
    fn read(&self, _minor: u32, _buf: &mut [u8]) -> Result<usize, FsError> {
        // 读取总是返回 EOF
        Ok(0)
    }
    
    fn write(&self, _minor: u32, buf: &[u8]) -> Result<usize, FsError> {
        // 写入总是成功，数据丢弃
        Ok(buf.len())
    }
    
    fn ioctl(&self, _minor: u32, _request: u32, _arg: usize) 
        -> Result<isize, FsError> {
        Err(FsError::NotSupported)
    }
}

// 注册
pub fn init_null_device() {
    register_chrdev(1, Arc::new(NullDevice));
}
```

#### 内存磁盘设备

```rust
struct RamDisk {
    data: SpinLock<Vec<u8>>,
    block_size: usize,
}

impl RamDisk {
    fn new(size: usize, block_size: usize) -> Self {
        Self {
            data: SpinLock::new(vec![0; size]),
            block_size,
        }
    }
}

impl BlockDeviceDriver for RamDisk {
    fn block_size(&self) -> usize {
        self.block_size
    }
    
    fn total_blocks(&self, _minor: u32) -> usize {
        let data = self.data.lock();
        data.len() / self.block_size
    }
    
    fn read_at(&self, _minor: u32, offset: usize, buf: &mut [u8]) 
        -> Result<usize, FsError> {
        let data = self.data.lock();
        if offset >= data.len() {
            return Ok(0);
        }
        
        let len = core::cmp::min(buf.len(), data.len() - offset);
        buf[..len].copy_from_slice(&data[offset..offset + len]);
        Ok(len)
    }
    
    fn write_at(&self, _minor: u32, offset: usize, buf: &[u8]) 
        -> Result<usize, FsError> {
        let mut data = self.data.lock();
        if offset >= data.len() {
            return Err(FsError::InvalidArgument);
        }
        
        let len = core::cmp::min(buf.len(), data.len() - offset);
        data[offset..offset + len].copy_from_slice(&buf[..len]);
        Ok(len)
    }
    
    // read_block 和 write_block 实现...
}
```

### 常见设备号分配

| 主设备号 | 类型 | 设备名 | 说明 |
|---------|------|--------|------|
| 1 | 字符 | mem | 内存设备 (/dev/null, /dev/zero) |
| 4 | 字符 | tty | 终端设备 |
| 5 | 字符 | tty | 控制台 |
| 8 | 块 | sd | SCSI 磁盘 (/dev/sda, /dev/sdb) |
| 11 | 块 | sr | SCSI CD-ROM |

## 使用场景

### 文件锁场景

#### 数据库锁

```rust
// 数据库文件锁
pub fn db_transaction() -> Result<(), FsError> {
    let db_file = open_db()?;
    
    // 获取写锁
    acquire_write_lock(&db_file)?;
    
    // 执行事务
    // ...
    
    // 释放锁
    release_lock(&db_file)?;
    Ok(())
}
```

#### 日志轮转

```rust
// 多进程写日志，使用读锁
pub fn write_log(msg: &str) -> Result<(), FsError> {
    let log_file = open_log()?;
    
    acquire_write_lock(&log_file)?;
    log_file.write(msg.as_bytes())?;
    release_lock(&log_file)?;
    
    Ok(())
}
```

### 设备访问场景

#### 读取磁盘分区

```rust
// 读取 /dev/sda1 第一个扇区
pub fn read_boot_sector() -> Result<Vec<u8>, FsError> {
    let dentry = vfs_lookup("/dev/sda1")?;
    let file = Arc::new(RegFile::new(dentry, OpenFlags::O_RDONLY));
    
    let mut buf = vec![0u8; 512];
    file.read(&mut buf)?;
    Ok(buf)
}
```

#### 写入 /dev/null

```rust
// 丢弃输出
pub fn discard_output(data: &[u8]) -> Result<(), FsError> {
    let dentry = vfs_lookup("/dev/null")?;
    let file = Arc::new(RegFile::new(dentry, OpenFlags::O_WRONLY));
    
    file.write(data)?;
    Ok(())
}
```

## 最佳实践

### 文件锁

1. **总是释放锁**: 使用 RAII 模式确保锁被释放
2. **避免死锁**: 按固定顺序获取多个锁
3. **最小锁范围**: 只锁定必要的文件范围
4. **超时机制**: 使用非阻塞模式并重试

### 设备驱动

1. **错误处理**: 硬件操作可能失败，正确处理错误
2. **同步**: 设备访问需要同步保护
3. **缓存**: 考虑实现设备缓存提高性能
4. **中断**: 使用中断驱动而不是轮询

## 相关资源

### 源代码位置

- **文件锁**: `os/src/vfs/file_lock.rs`
- **设备号工具**: `os/src/vfs/dev.rs`
- **设备驱动注册**: `os/src/vfs/devno.rs`
- **CharDevFile**: `os/src/vfs/impls/char_dev_file.rs`
- **BlkDevFile**: `os/src/vfs/impls/blk_dev_file.rs`

### 参考文档

- [VFS 整体架构](architecture.md)
- [File 与 FDTable](file_and_fdtable.md)
- [使用指南](usage.md)
