# VFS 使用指南

## 概述

本文档提供 VFS 子系统的实用指南，包括常见使用场景、代码示例、最佳实践和故障排查。适合开发者快速上手 VFS API，了解如何在内核中进行文件操作。

## 快速开始

### 初始化 VFS

系统启动时需要挂载根文件系统：

```rust
// 在 os/src/main.rs 中
pub fn init_vfs() -> Result<(), FsError> {
    // 1. 创建根文件系统 (tmpfs)
    let tmpfs = TmpFs::new();
    let fs: Arc<dyn FileSystem> = Arc::new(tmpfs);
    
    // 2. 挂载到根目录
    vfs::MOUNT_TABLE.mount(
        fs,
        "/",
        MountFlags::empty(),
        None
    )?;
    
    // 3. 创建基本目录结构
    let root = vfs::get_root_dentry()?;
    root.inode.mkdir("dev", FileMode::S_IFDIR | FileMode::S_IRWXU)?;
    root.inode.mkdir("etc", FileMode::S_IFDIR | FileMode::S_IRWXU)?;
    root.inode.mkdir("tmp", FileMode::S_IFDIR | FileMode::S_IRWXU)?;
    root.inode.mkdir("mnt", FileMode::S_IFDIR | FileMode::S_IRWXU)?;
    
    Ok(())
}
```

### 初始化进程文件描述符

每个进程启动时初始化标准 I/O：

```rust
pub fn init_stdio(task: &Task) -> Result<(), FsError> {
    let fd_table = &task.fd_table;
    
    // 创建标准 I/O 文件
    let (stdin, stdout, stderr) = vfs::create_stdio_files();
    
    // 安装到文件描述符 0, 1, 2
    fd_table.install_at(0, stdin)?;   // stdin
    fd_table.install_at(1, stdout)?;  // stdout
    fd_table.install_at(2, stderr)?;  // stderr
    
    Ok(())
}
```

## 常见操作

### 文件操作

#### 打开文件

```rust
use vfs::{vfs_lookup, RegFile, OpenFlags};

pub fn sys_open(path: &str, flags: OpenFlags, mode: FileMode) 
    -> Result<usize, FsError> {
    // 1. 解析路径
    let dentry = if flags.contains(OpenFlags::O_CREAT) {
        // 创建文件
        let (dir, name) = vfs::split_path(path)?;
        let parent = vfs::vfs_lookup(&dir)?;
        
        match parent.inode.lookup(&name) {
            Ok(inode) => {
                if flags.contains(OpenFlags::O_EXCL) {
                    return Err(FsError::AlreadyExists);
                }
                Dentry::new(name, inode)
            }
            Err(FsError::NotFound) => {
                let inode = parent.inode.create(&name, mode)?;
                let dentry = Dentry::new(name, inode);
                parent.add_child(dentry.clone());
                dentry
            }
            Err(e) => return Err(e),
        }
    } else {
        vfs::vfs_lookup(path)?
    };
    
    // 2. 创建 File 对象
    let file = Arc::new(RegFile::new(dentry, flags));
    
    // 3. 如果是 O_TRUNC，截断文件
    if flags.contains(OpenFlags::O_TRUNC) {
        file.inode()?.truncate(0)?;
    }
    
    // 4. 分配文件描述符
    let current = current_task();
    let fd_table = &current.lock().fd_table;
    let fd_flags = FdFlags::from_open_flags(flags);
    let fd = fd_table.alloc_with_flags(file, fd_flags)?;
    
    Ok(fd)
}
```

#### 读取文件

```rust
pub fn sys_read(fd: usize, buf: &mut [u8]) -> Result<usize, FsError> {
    let current = current_task();
    let fd_table = &current.lock().fd_table;
    
    let file = fd_table.get(fd)?;
    if !file.readable() {
        return Err(FsError::PermissionDenied);
    }
    
    file.read(buf)
}
```

#### 写入文件

```rust
pub fn sys_write(fd: usize, buf: &[u8]) -> Result<usize, FsError> {
    let current = current_task();
    let fd_table = &current.lock().fd_table;
    
    let file = fd_table.get(fd)?;
    if !file.writable() {
        return Err(FsError::PermissionDenied);
    }
    
    file.write(buf)
}
```

#### 关闭文件

```rust
pub fn sys_close(fd: usize) -> Result<(), FsError> {
    let current = current_task();
    let fd_table = &current.lock().fd_table;
    fd_table.close(fd)
}
```

#### Seek 操作

```rust
pub fn sys_lseek(fd: usize, offset: isize, whence: SeekWhence) 
    -> Result<usize, FsError> {
    let current = current_task();
    let fd_table = &current.lock().fd_table;
    
    let file = fd_table.get(fd)?;
    file.lseek(offset, whence)
}
```

### 目录操作

#### 创建目录

```rust
pub fn sys_mkdir(path: &str, mode: FileMode) -> Result<(), FsError> {
    let (dir, name) = vfs::split_path(path)?;
    let parent = vfs::vfs_lookup(&dir)?;
    parent.inode.mkdir(&name, mode | FileMode::S_IFDIR)?;
    Ok(())
}
```

#### 删除目录

```rust
pub fn sys_rmdir(path: &str) -> Result<(), FsError> {
    let (dir, name) = vfs::split_path(path)?;
    let parent = vfs::vfs_lookup(&dir)?;
    
    // 删除 inode
    parent.inode.rmdir(&name)?;
    
    // 清理缓存
    parent.remove_child(&name);
    vfs::DENTRY_CACHE.remove(path);
    
    Ok(())
}
```

#### 读取目录

```rust
pub fn sys_getdents64(fd: usize, buf: &mut [u8]) -> Result<usize, FsError> {
    let current = current_task();
    let fd_table = &current.lock().fd_table;
    
    let file = fd_table.get(fd)?;
    let dentry = file.dentry()?;
    
    // 获取目录项列表
    let entries = dentry.inode.readdir()?;
    
    // 序列化到缓冲区
    let mut offset = 0;
    for entry in entries {
        let dirent = LinuxDirent64 {
            d_ino: entry.inode_no as u64,
            d_off: offset as i64,
            d_reclen: /* 计算记录长度 */,
            d_type: inode_type_to_d_type(entry.inode_type),
            d_name: entry.name,
        };
        
        // 写入缓冲区
        offset += dirent.write_to(&mut buf[offset..])?;
    }
    
    Ok(offset)
}
```

#### 切换工作目录

```rust
pub fn sys_chdir(path: &str) -> Result<(), FsError> {
    let dentry = vfs::vfs_lookup(path)?;
    
    // 检查是否是目录
    let metadata = dentry.inode.metadata()?;
    if metadata.inode_type != InodeType::Directory {
        return Err(FsError::NotDirectory);
    }
    
    // 更新当前工作目录
    let current = current_task();
    current.lock().fs.lock().cwd = Some(dentry);
    
    Ok(())
}
```

### 链接操作

#### 创建硬链接

```rust
pub fn sys_link(oldpath: &str, newpath: &str) -> Result<(), FsError> {
    // 查找源文件
    let old_dentry = vfs::vfs_lookup(oldpath)?;
    
    // 解析目标路径
    let (dir, name) = vfs::split_path(newpath)?;
    let parent = vfs::vfs_lookup(&dir)?;
    
    // 创建硬链接
    parent.inode.link(&name, &old_dentry.inode)?;
    
    Ok(())
}
```

#### 删除链接

```rust
pub fn sys_unlink(path: &str) -> Result<(), FsError> {
    let (dir, name) = vfs::split_path(path)?;
    let parent = vfs::vfs_lookup(&dir)?;
    
    // 删除链接
    parent.inode.unlink(&name)?;
    
    // 清理缓存
    parent.remove_child(&name);
    vfs::DENTRY_CACHE.remove(path);
    
    Ok(())
}
```

#### 创建符号链接

```rust
pub fn sys_symlink(target: &str, linkpath: &str) -> Result<(), FsError> {
    let (dir, name) = vfs::split_path(linkpath)?;
    let parent = vfs::vfs_lookup(&dir)?;
    
    parent.inode.symlink(&name, target)?;
    Ok(())
}
```

#### 读取符号链接

```rust
pub fn sys_readlink(path: &str, buf: &mut [u8]) -> Result<usize, FsError> {
    let dentry = vfs::vfs_lookup_no_follow(path)?;
    
    let target = dentry.inode.readlink()?;
    let len = core::cmp::min(buf.len(), target.len());
    buf[..len].copy_from_slice(&target.as_bytes()[..len]);
    
    Ok(len)
}
```

### 管道操作

#### 创建管道

```rust
pub fn sys_pipe() -> Result<(usize, usize), FsError> {
    let (read_file, write_file) = vfs::create_pipe()?;
    
    let current = current_task();
    let fd_table = &current.lock().fd_table;
    
    let read_fd = fd_table.alloc(read_file)?;
    let write_fd = fd_table.alloc(write_file)?;
    
    Ok((read_fd, write_fd))
}
```

#### 使用管道通信

父子进程通过管道通信：

```rust
pub fn pipe_example() -> Result<(), FsError> {
    // 创建管道
    let (read_fd, write_fd) = sys_pipe()?;
    
    // fork 子进程
    if sys_fork()? == 0 {
        // 子进程：关闭写端，读取数据
        sys_close(write_fd)?;
        
        let mut buf = [0u8; 128];
        let n = sys_read(read_fd, &mut buf)?;
        // 处理数据...
        
        sys_exit(0);
    } else {
        // 父进程：关闭读端，写入数据
        sys_close(read_fd)?;
        
        sys_write(write_fd, b"Hello, child!")?;
        sys_close(write_fd)?;
        
        sys_wait()?;
    }
    
    Ok(())
}
```

### 文件描述符操作

#### dup/dup2

```rust
// 重定向标准输出到文件
pub fn redirect_stdout(path: &str) -> Result<(), FsError> {
    // 打开目标文件
    let fd = sys_open(path, 
        OpenFlags::O_WRONLY | OpenFlags::O_CREAT | OpenFlags::O_TRUNC,
        FileMode::S_IRUSR | FileMode::S_IWUSR)?;
    
    // 复制到 stdout (fd 1)
    let current = current_task();
    let fd_table = &current.lock().fd_table;
    fd_table.dup2(fd, 1)?;
    fd_table.close(fd)?;
    
    Ok(())
}
```

#### fcntl 操作

```rust
pub fn sys_fcntl(fd: usize, cmd: u32, arg: usize) -> Result<isize, FsError> {
    let current = current_task();
    let fd_table = &current.lock().fd_table;
    
    match cmd {
        F_GETFD => {
            // 获取 FD 标志
            let flags = fd_table.get_fd_flags(fd)?;
            Ok(flags.bits() as isize)
        }
        F_SETFD => {
            // 设置 FD 标志
            let flags = FdFlags::from_bits_truncate(arg as u32);
            fd_table.set_fd_flags(fd, flags)?;
            Ok(0)
        }
        F_GETFL => {
            // 获取文件状态标志
            let file = fd_table.get(fd)?;
            Ok(file.flags().bits() as isize)
        }
        F_SETFL => {
            // 设置文件状态标志
            let file = fd_table.get(fd)?;
            let flags = OpenFlags::from_bits_truncate(arg as u32);
            file.set_status_flags(flags)?;
            Ok(0)
        }
        _ => Err(FsError::NotSupported)
    }
}
```

### 挂载操作

#### 挂载文件系统

```rust
pub fn sys_mount(device: &str, path: &str, fstype: &str, flags: u32) 
    -> Result<(), FsError> {
    // 创建文件系统实例
    let fs: Arc<dyn FileSystem> = match fstype {
        "tmpfs" => Arc::new(TmpFs::new()),
        "fat32" => {
            let dev = vfs::vfs_lookup(device)?;
            Arc::new(Fat32Fs::new(dev)?)
        }
        _ => return Err(FsError::NotSupported),
    };
    
    // 挂载
    let mount_flags = MountFlags::from_bits_truncate(flags);
    vfs::MOUNT_TABLE.mount(
        fs,
        path,
        mount_flags,
        Some(String::from(device))
    )?;
    
    Ok(())
}
```

#### 卸载文件系统

```rust
pub fn sys_umount(path: &str) -> Result<(), FsError> {
    vfs::MOUNT_TABLE.umount(path)
}
```

## 最佳实践

### 资源管理

#### 使用 RAII 模式

```rust
struct FileGuard {
    fd: usize,
    fd_table: Arc<FDTable>,
}

impl FileGuard {
    fn new(path: &str, flags: OpenFlags) -> Result<Self, FsError> {
        let fd = sys_open(path, flags, FileMode::empty())?;
        let current = current_task();
        let fd_table = current.lock().fd_table.clone();
        Ok(Self { fd, fd_table })
    }
}

impl Drop for FileGuard {
    fn drop(&mut self) {
        let _ = self.fd_table.close(self.fd);
    }
}

// 使用
{
    let file = FileGuard::new("/tmp/test", OpenFlags::O_RDONLY)?;
    // 使用文件...
}  // 自动关闭
```

#### 批量操作

对同一目录下的多个文件，先查找目录 Dentry：

```rust
pub fn batch_create_files(dir: &str, names: &[&str]) 
    -> Result<(), FsError> {
    // 一次查找目录
    let parent = vfs::vfs_lookup(dir)?;
    
    // 批量创建文件
    for name in names {
        parent.inode.create(name, 
            FileMode::S_IFREG | FileMode::S_IRUSR | FileMode::S_IWUSR)?;
    }
    
    Ok(())
}
```

### 错误处理

#### 正确处理错误

```rust
pub fn robust_file_read(path: &str) -> Result<Vec<u8>, String> {
    // 打开文件
    let dentry = vfs::vfs_lookup(path)
        .map_err(|e| format!("Failed to lookup {}: {:?}", path, e))?;
    
    let file = Arc::new(RegFile::new(dentry, OpenFlags::O_RDONLY));
    
    // 获取文件大小
    let metadata = file.metadata()
        .map_err(|e| format!("Failed to get metadata: {:?}", e))?;
    
    // 分配缓冲区
    let mut buf = vec![0u8; metadata.size];
    
    // 读取数据
    let mut offset = 0;
    while offset < metadata.size {
        let n = file.read(&mut buf[offset..])
            .map_err(|e| format!("Failed to read at {}: {:?}", offset, e))?;
        
        if n == 0 {
            break;  // EOF
        }
        offset += n;
    }
    
    buf.truncate(offset);
    Ok(buf)
}
```

### 性能优化

#### 大文件读写

使用大缓冲区减少系统调用：

```rust
pub fn copy_file(src: &str, dst: &str) -> Result<(), FsError> {
    const BUF_SIZE: usize = 64 * 1024;  // 64KB 缓冲区
    
    let src_fd = sys_open(src, OpenFlags::O_RDONLY, FileMode::empty())?;
    let dst_fd = sys_open(dst, 
        OpenFlags::O_WRONLY | OpenFlags::O_CREAT | OpenFlags::O_TRUNC,
        FileMode::S_IRUSR | FileMode::S_IWUSR)?;
    
    let mut buf = vec![0u8; BUF_SIZE];
    
    loop {
        let n = sys_read(src_fd, &mut buf)?;
        if n == 0 {
            break;
        }
        
        sys_write(dst_fd, &buf[..n])?;
    }
    
    sys_close(src_fd)?;
    sys_close(dst_fd)?;
    
    Ok(())
}
```

#### 使用 pread/pwrite

避免 lseek + read/write 的竞争条件：

```rust
pub fn read_at_offset(file: &Arc<dyn File>, offset: usize, buf: &mut [u8]) 
    -> Result<usize, FsError> {
    // 一次调用，不改变文件 offset
    file.read_at(offset, buf)
}
```

## 常见陷阱

### 1. 忘记关闭文件描述符

**错误**:
```rust
for i in 0..1000 {
    let fd = sys_open("/tmp/test", OpenFlags::O_RDONLY, FileMode::empty())?;
    // 忘记 close，导致 fd 泄漏
}
```

**正确**:
```rust
for i in 0..1000 {
    let fd = sys_open("/tmp/test", OpenFlags::O_RDONLY, FileMode::empty())?;
    // 使用文件...
    sys_close(fd)?;
}
```

### 2. dup 后的竞争条件

**错误**:
```rust
let fd1 = sys_open("/tmp/file", OpenFlags::O_RDWR, FileMode::empty())?;
let fd2 = sys_dup(fd1)?;

// 两个线程同时使用 fd1 和 fd2，共享 offset，导致读写混乱
```

**正确**:
```rust
// 如果需要独立的 offset，重新打开文件
let fd1 = sys_open("/tmp/file", OpenFlags::O_RDWR, FileMode::empty())?;
let fd2 = sys_open("/tmp/file", OpenFlags::O_RDWR, FileMode::empty())?;
```

### 3. 路径越界

**错误**:
```rust
// 可能越过根目录
let path = "/../../../etc/passwd";
```

**正确**:
```rust
// 使用 normalize_path 规范化
let path = vfs::normalize_path("/../../../etc/passwd");  // "/"
```

### 4. 缓存不一致

**错误**:
```rust
let dentry = vfs::vfs_lookup("/tmp/file")?;
dentry.inode.unlink("subfile")?;
// 忘记清理缓存，后续查找可能仍然找到已删除的文件
```

**正确**:
```rust
let dentry = vfs::vfs_lookup("/tmp/file")?;
dentry.inode.unlink("subfile")?;
dentry.remove_child("subfile");
vfs::DENTRY_CACHE.remove("/tmp/file/subfile");
```

## 故障排查

### 常见错误

#### NotFound

**原因**: 文件或目录不存在

**解决**: 检查路径是否正确，父目录是否存在

#### PermissionDenied

**原因**: 没有读/写权限

**解决**: 检查文件 mode 和打开标志 (O_RDONLY/O_WRONLY/O_RDWR)

#### IsDirectory

**原因**: 对目录执行了文件操作

**解决**: 使用 metadata() 检查文件类型

#### NotDirectory

**原因**: 对文件执行了目录操作

**解决**: 确保操作对象是目录

#### TooManyOpenFiles

**原因**: 超过最大文件描述符限制

**解决**: 关闭不需要的文件，或增加 `DEFAULT_MAX_FDS`

#### FileExists

**原因**: 文件已存在 (O_CREAT | O_EXCL)

**解决**: 检查是否应该使用 O_TRUNC 覆盖

### 调试技巧

#### 打印文件描述符表

```rust
pub fn dump_fd_table() {
    let current = current_task();
    let fd_table = &current.lock().fd_table;
    println!("{:?}", fd_table);
}
```

#### 列出挂载点

```rust
pub fn list_mounts() {
    let mounts = vfs::MOUNT_TABLE.list_mounts();
    for (path, fstype) in mounts {
        println!("{} on {} type {}", path, path, fstype);
    }
}
```

#### 跟踪路径解析

在 `vfs_lookup` 中添加日志：

```rust
pr_debug!(\"Looking up: {}\", path);
pr_debug!(\"Current dentry: {}\", current_dentry.name);
```

## 进阶主题

### 实现自定义文件系统

参考 tmpfs 实现自己的文件系统：

```rust
pub struct MyFs {
    // 文件系统状态...
}

impl FileSystem for MyFs {
    fn root_inode(&self) -> Arc<dyn Inode> {
        // 返回根 Inode
    }
    
    fn sync(&self) -> Result<(), FsError> {
        // 同步数据到持久化存储
    }
    
    fn umount(&self) -> Result<(), FsError> {
        // 卸载清理
    }
    
    fn fs_type(&self) -> &str {
        "myfs"
    }
}
```

### 实现自定义文件类型

实现 File trait 创建特殊文件类型：

```rust
pub struct MyFile {
    // 文件状态...
}

impl File for MyFile {
    fn readable(&self) -> bool { true }
    fn writable(&self) -> bool { true }
    
    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        // 自定义读取逻辑
    }
    
    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        // 自定义写入逻辑
    }
    
    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        // 返回元数据
    }
}
```

## 相关资源

### 源代码示例

- **系统调用实现**: `os/src/kernel/syscall/fs.rs`
- **tmpfs 实现**: `os/src/fs/tmpfs/`
- **fat32 实现**: `os/src/fs/fat32/`

### 参考文档

- [VFS 整体架构](architecture.md)
- [Inode 与 Dentry](inode_and_dentry.md)
- [File 与 FDTable](file_and_fdtable.md)
- [路径解析与挂载](path_and_mount.md)

### 相关系统调用

| 系统调用 | 功能 | 对应 VFS API |
|---------|------|--------------|
| open | 打开文件 | `vfs_lookup` + `RegFile::new` |
| read | 读取 | `File::read` |
| write | 写入 | `File::write` |
| close | 关闭 | `FDTable::close` |
| lseek | 定位 | `File::lseek` |
| stat | 获取元数据 | `Inode::metadata` |
| mkdir | 创建目录 | `Inode::mkdir` |
| rmdir | 删除目录 | `Inode::rmdir` |
| link | 硬链接 | `Inode::link` |
| unlink | 删除文件 | `Inode::unlink` |
| symlink | 符号链接 | `Inode::symlink` |
| readlink | 读链接 | `Inode::readlink` |
| mount | 挂载 | `MountTable::mount` |
| umount | 卸载 | `MountTable::umount` |
| dup | 复制 FD | `FDTable::dup` |
| dup2 | 复制到指定 FD | `FDTable::dup2` |
| pipe | 创建管道 | `create_pipe` |
| chdir | 切换目录 | 更新 `cwd` |
