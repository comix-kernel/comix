# VFS 子系统文档

## 简介

VFS (Virtual File System) 是 Comix 内核的虚拟文件系统层,提供统一的文件系统抽象接口。该系统采用分层设计,支持多种文件类型和文件系统,为上层系统调用和下层具体文件系统实现之间搭建了桥梁。

VFS 的核心特点是**分层抽象设计**:将文件访问分为会话层和存储层,会话层维护打开文件的状态(如偏移量、标志),存储层提供无状态的随机访问接口。这种设计使得相同的底层 Inode 可以被多个进程以不同的方式访问,既保证了数据共享,又维护了各自独立的会话状态。

作为内核文件系统的核心基础设施,VFS 子系统支持路径解析、挂载管理、[目录项缓存和文件描述符管理等完整功能。它采用目录项缓存加速路径查找,使用挂载表实现灵活的文件系统组织,并通过文件描述符表为每个进程提供独立的文件视图。

### 主要功能

- **分层文件抽象**:会话层 (File trait) 和存储层 (Inode trait) 分离,清晰的职责划分
- **多文件类型支持**:普通文件、管道、字符设备、块设备、符号链接、FIFO、Socket
- **路径解析**:支持绝对路径和相对路径,自动处理 `.` 和 `..`,符号链接解析
- **目录项缓存**:Dentry 缓存加速重复路径查找,减少磁盘访问
- **挂载管理**:支持多文件系统挂载,挂载点栈,灵活的文件系统组织
- **文件描述符表**:进程级文件描述符管理,支持 dup/dup2/dup3,close-on-exec 标志
- **文件锁**:支持 POSIX 文件锁 (flock),进程间文件访问同步
- **设备文件**:字符设备和块设备抽象,设备号管理
- **标准化接口**:与 POSIX 兼容的文件操作接口,权限管理,元数据访问

### 模块结构

```
os/src/vfs/
├── mod.rs              # 模块入口,导出公共 API
├── file.rs             # File trait 定义 (会话层接口)
├── inode.rs            # Inode trait 定义 (存储层接口)
├── dentry.rs           # 目录项结构和全局缓存
├── path.rs             # 路径解析和查找逻辑
├── mount.rs            # 挂载表和挂载点管理
├── fd_table.rs         # 文件描述符表
├── file_lock.rs        # 文件锁管理器
├── file_system.rs      # FileSystem trait 定义
├── adapter.rs          # 类型转换适配器
├── dev.rs              # 设备号工具函数
├── devno.rs            # 设备驱动注册表
├── error.rs            # VFS 错误类型定义
└── impls/              # 具体文件类型实现
    ├── reg_file.rs     # 普通文件
    ├── pipe_file.rs    # 管道文件
    ├── stdio_file.rs   # 标准 I/O 文件
    ├── char_dev_file.rs # 字符设备文件
    └── blk_dev_file.rs  # 块设备文件
```

### 模块职责

- **mod.rs**:模块的统一入口,导出所有公共 API 和类型,提供便利函数如 `vfs_load_elf`
- **file.rs**:定义会话层接口 `File` trait,声明 read、write、lseek 等方法,支持可选方法提供默认实现
- **inode.rs**:定义存储层接口 `Inode` trait,提供 read_at、write_at、lookup、create、mkdir 等无状态方法
- **dentry.rs**:实现目录项 (Dentry) 结构,维护文件名到 Inode 的映射,管理父子关系和全局缓存
- **path.rs**:实现路径解析引擎,处理绝对/相对路径、`.` 和 `..`、符号链接,提供 `vfs_lookup` 等 API
- **mount.rs**:管理文件系统挂载,维护全局挂载表,支持挂载点栈和最长前缀匹配
- **fd_table.rs**:实现进程级文件描述符表,支持分配、关闭、复制文件描述符,管理 close-on-exec 标志
- **file_lock.rs**:实现全局文件锁管理器,支持共享锁和排他锁,死锁检测
- **file_system.rs**:定义文件系统抽象接口 `FileSystem` trait,声明 root_inode、sync、umount 等方法
- **impls/**:包含各种具体文件类型的实现,如 RegFile (基于 Inode)、PipeFile (环形缓冲区)、StdioFile 等

## 文档导航

### 核心概念

- **[整体架构](architecture.md)**:VFS 子系统的分层架构、模块依赖、数据流转、设计决策和性能考量

### 子模块详解

- **[Inode与Dentry](inode_and_dentry.md)**:存储层 Inode 接口、目录项 Dentry 结构、缓存机制和生命周期管理
- **[File与FDTable](file_and_fdtable.md)**:会话层 File 接口、文件描述符表实现、文件类型详解
- **[路径解析与挂载](path_and_mount.md)**:路径解析算法、挂载表管理、符号链接处理、挂载点查找
- **[FileSystem与错误处理](filesystem_and_errors.md)**:FileSystem trait 接口、错误类型系统、文件系统实现指南
- **[文件锁与设备管理](filelock_and_devices.md)**:POSIX 文件锁、设备驱动注册、设备文件操作

### 使用指南

- **[使用指南](usage.md)**:VFS 系统的基本使用、文件操作、路径查找、挂载管理、最佳实践和常见陷阱

## 设计原则

VFS 子系统的设计遵循以下核心原则:

### 1. 分层抽象

采用会话层和存储层分离的设计:
- **会话层 (File)**:维护打开文件的状态,如当前偏移量、打开标志,支持有状态的 read/write 操作
- **存储层 (Inode)**:提供无状态的随机访问接口,所有方法携带 offset 参数,可被多个 File 共享

这种分层使得同一个文件可以被多个进程或多次打开,每次打开都有独立的会话状态,但共享底层存储。

### 2. 统一接口

通过 trait 定义统一的文件操作接口,屏蔽不同文件类型的实现细节:
- 所有文件类型都实现 `File` trait,可以统一存储在 `Arc<dyn File>` 中
- 所有存储对象都实现 `Inode` trait,可以统一处理文件、目录、设备等
- 文件描述符表对文件类型一无所知,完全通过 trait 对象操作

### 3. 缓存优化

使用多级缓存减少重复计算和磁盘访问:
- **Dentry 缓存**:缓存路径到 Dentry 的映射,避免重复路径解析
- **Dentry 树缓存**:父子关系缓存在 Dentry 内部,加速相对路径查找
- **挂载点缓存**:每个 Dentry 缓存其挂载点信息,避免每次查挂载表

### 4. 引用计数管理

使用 Rust 的 `Arc` 和 `Weak` 智能指针管理对象生命周期:
- Dentry 使用 `Arc` 共享所有权,`Weak` 避免父子循环引用
- Inode 由 Dentry 持有 `Arc`,可被多个 Dentry 共享 (硬链接)
- File 对象由文件描述符表持有 `Arc`,支持 dup 等操作共享

## 重要约定

### 会话层与存储层的区别

| 方面 | File (会话层) | Inode (存储层) |
|------|---------------|----------------|
| 职责 | 维护打开文件的状态 | 提供底层存储访问 |
| 状态 | 有状态 (offset、flags) | 无状态 |
| 方法 | `read(buf)`, `write(buf)` | `read_at(offset, buf)`, `write_at(offset, buf)` |
| 实例 | 每次 open 创建新实例 | 多个 File 可共享同一个 Inode |
| 存储位置 | 文件描述符表 | Dentry 中 |

### 路径解析规则

- **绝对路径**:以 `/` 开头,从根目录开始解析
- **相对路径**:不以 `/` 开头,从当前工作目录开始
- **`.` 组件**:表示当前目录,解析时跳过
- **`..` 组件**:表示父目录,绝对路径中不能越过根目录,相对路径中累积 `..`
- **符号链接**:`vfs_lookup` 自动跟随,`vfs_lookup_no_follow` 不跟随最后一个组件

### 挂载点处理

- 挂载表使用**最长前缀匹配**:如果 `/mnt/data` 和 `/mnt` 都是挂载点,访问 `/mnt/data/file` 使用 `/mnt/data` 的挂载点
- 支持**挂载点栈**:同一路径可以多次挂载,最后挂载的文件系统覆盖之前的
- **自动跟随挂载点**:`vfs_lookup` 在解析路径时自动切换到挂载点的根 Dentry

### 文件描述符约定

- **FD 0-2 预留**:0 = stdin, 1 = stdout, 2 = stderr
- **最小可用 FD**:alloc() 总是分配最小的可用文件描述符
- **dup 语义**:dup 复制的 FD 指向同一个 `Arc<dyn File>`,共享偏移量
- **close-on-exec**:`O_CLOEXEC` 和 `FD_CLOEXEC` 标志控制 exec 时是否关闭文件

## 快速开始

### 打开和读取文件

```rust
use vfs::{vfs_lookup, RegFile, OpenFlags};
use alloc::sync::Arc;

// 1. 查找文件路径
let dentry = vfs_lookup(\"/etc/passwd\")?;

// 2. 创建 RegFile (普通文件)
let file = Arc::new(RegFile::new(dentry, OpenFlags::O_RDONLY));

// 3. 读取数据
let mut buf = [0u8; 1024];
let n = file.read(&mut buf)?;
```

### 使用文件描述符

```rust
use vfs::FDTable;

// 创建文件描述符表
let fd_table = FDTable::new();

// 分配文件描述符
let fd = fd_table.alloc(file)?;

// 从文件描述符读取
let file = fd_table.get(fd)?;
let n = file.read(&mut buf)?;

// 关闭文件描述符
fd_table.close(fd)?;
```

### 路径操作

```rust
use vfs::{normalize_path, split_path, parse_path};

// 规范化路径
let path = normalize_path(\"/a/b/../c/./d\");  // \"/a/c/d\"

// 分割目录和文件名
let (dir, name) = split_path(\"/etc/passwd\")?;  // (\"/etc\", \"passwd\")

// 解析路径组件
let components = parse_path(\"../../foo/bar\");
```

### 挂载文件系统

```rust
use vfs::{MOUNT_TABLE, MountFlags};
use alloc::sync::Arc;

// 假设已有一个文件系统实现
let fs: Arc<dyn FileSystem> = create_my_fs()?;

// 挂载到 /mnt
MOUNT_TABLE.mount(
    fs,
    \"/mnt\",
    MountFlags::empty(),
    Some(String::from(\"/dev/sda1\"))
)?;

// 访问挂载点下的文件
let dentry = vfs_lookup(\"/mnt/data/file.txt\")?;

// 卸载
MOUNT_TABLE.umount(\"/mnt\")?;
```

## 相关资源

### 源代码位置

- **主模块**:`os/src/vfs/mod.rs`
- **核心接口**:`os/src/vfs/file.rs`, `os/src/vfs/inode.rs`
- **路径解析**:`os/src/vfs/path.rs`
- **完整源码**:`os/src/vfs/` 目录

### 配置常量

- **最大文件描述符数**:`os/src/config.rs:DEFAULT_MAX_FDS`

### 依赖模块

- **sync**:提供 `SpinLock` 等同步原语
- **kernel**:提供 `current_task()` 获取当前任务信息
- **uapi**:定义 POSIX 兼容的类型和常量 (OpenFlags, Stat, etc.)

### 支持的文件系统

- **tmpfs**:内存文件系统
- **fat32**:FAT32 文件系统 (通过 fatfs crate)
- **devfs**:设备文件系统 (字符设备和块设备)

### 版本信息

- **Rust 版本**:nightly-2025-01-13
- **目标架构**:riscv64gc-unknown-none-elf
- **支持架构**:RISC-V (当前),LoongArch (规划中)
