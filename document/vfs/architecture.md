# VFS 子系统架构

## 概述

本文档详细介绍 VFS 子系统的整体架构、模块依赖关系、数据流转过程、设计决策以及性能和安全性考量。VFS 子系统采用分层架构设计,各层职责清晰,通过目录项缓存和挂载表实现高效的文件访问。

## 分层架构

VFS 子系统采用四层架构,从上到下依次为应用层、路径层、会话层和存储层:

```
┌─────────────────────────────────────────────────────────────────┐
│                      应用层 (Application Layer)                  │
│                                                                   │
│    系统调用: open() read() write() lseek() close() mount()      │
│              getdents64() stat() fstat() chown() chmod()         │
│                                                                   │
│    文件描述符表 (FDTable)                                        │
│    进程级资源,管理打开的文件,支持 dup/dup2/close-on-exec       │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              │ 通过 FD 获取 Arc<dyn File>
                              │
┌─────────────────────────────▼───────────────────────────────────┐
│                       会话层 (Session Layer)                     │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  File trait - 有状态的文件操作接口                      │    │
│  │                                                           │    │
│  │  · read(buf) / write(buf) - 从当前 offset 读写          │    │
│  │  · lseek(offset, whence) - 设置偏移量                   │    │
│  │  · metadata() - 获取文件元数据                          │    │
│  │                                                           │    │
│  │  实现类型:                                               │    │
│  │  · RegFile - 普通文件 (基于 Inode,支持 seek)           │    │
│  │  · PipeFile - 管道 (环形缓冲区,流式)                    │    │
│  │  · StdioFile - 标准 I/O                                  │    │
│  │  · CharDevFile - 字符设备文件                           │    │
│  │  · BlkDevFile - 块设备文件                              │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              │ RegFile 持有 Dentry
                              │
┌─────────────────────────────▼───────────────────────────────────┐
│                       路径层 (Path Layer)                        │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Dentry (目录项) - 路径组件的缓存                       │    │
│  │                                                           │    │
│  │  · name: String - 文件名                                │    │
│  │  · inode: Arc<dyn Inode> - 关联的 Inode                │    │
│  │  · parent: Weak<Dentry> - 父目录 (弱引用避免循环)       │    │
│  │  · children: BTreeMap<String, Arc<Dentry>> - 子项缓存  │    │
│  │  · mount_point: Option<Weak<Dentry>> - 挂载点信息      │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  DentryCache - 全局路径缓存                             │    │
│  │  cache: BTreeMap<String, Weak<Dentry>>                 │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  MountTable - 全局挂载表                                │    │
│  │  mounts: BTreeMap<String, Vec<Arc<MountPoint>>>        │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              │ Dentry 持有 Inode
                              │
┌─────────────────────────────▼───────────────────────────────────┐
│                      存储层 (Storage Layer)                      │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Inode trait - 无状态的存储访问接口                     │    │
│  │                                                           │    │
│  │  · read_at(offset, buf) / write_at(offset, buf)         │    │
│  │  · metadata() - 获取文件元数据                          │    │
│  │  · lookup(name) - 在目录中查找子项                      │    │
│  │  · create / mkdir / unlink / rmdir - 目录操作           │    │
│  │  · readdir() - 列出目录内容                             │    │
│  │  · truncate / sync - 文件管理                           │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  FileSystem trait - 文件系统抽象                        │    │
│  │  · root_inode() - 获取根 Inode                          │    │
│  │  · sync() / umount() - 文件系统操作                     │    │
│  └─────────────────────────────────────────────────────────┘    │
└───────────────────────────────────────────────────────────────────┘
```

### 各层职责

#### 应用层 (Application Layer)

应用层是用户空间和内核空间的接口,通过系统调用提供文件操作功能。文件描述符表 (FDTable) 是进程级资源,每个进程维护独立的文件描述符空间。主要职责:
- 系统调用参数验证和权限检查
- 文件描述符到 File 对象的映射管理
- dup/dup2/dup3 文件描述符复制
- close-on-exec 标志管理 (用于 exec 系统调用)

#### 会话层 (Session Layer)

会话层维护打开文件的会话状态,如当前读写偏移量、打开标志 (只读/只写/读写/追加等)。同一个底层 Inode 可以被多个 File 对象引用,每个都有独立的 offset。主要特点:
- 方法不携带 offset 参数,由内部维护 (`read`/`write` vs `read_at`/`write_at`)
- 支持可 seek 文件 (RegFile) 和流式设备 (PipeFile)
- 通过 trait 对象 `Arc<dyn File>` 实现多态,支持异构文件类型

#### 路径层 (Path Layer)

路径层管理文件系统的命名空间,包括目录树结构、路径解析、挂载点管理。Dentry 是核心数据结构,缓存文件名到 Inode 的映射,加速重复路径查找。主要职责:
- 路径解析:支持绝对路径、相对路径、`.` 和 `..`
- 目录项缓存:避免重复的 Inode lookup 操作
- 挂载点管理:支持多文件系统挂载,最长前缀匹配
- 符号链接解析:自动跟随符号链接 (可选)

#### 存储层 (Storage Layer)

存储层提供无状态的文件存储访问接口,所有方法携带 offset 参数,实现随机访问能力。Inode 是抽象接口,具体实现由各个文件系统提供 (tmpfs、fat32 等)。主要职责:
- 数据读写:read_at/write_at 提供指定偏移量的访问
- 目录操作:lookup/create/mkdir/unlink/rmdir 管理目录结构
- 元数据管理:metadata/truncate/chmod/chown 等
- 文件系统操作:sync/umount 等全局操作

## 模块依赖关系

VFS 子系统内部模块之间的依赖关系如下图所示:

```
        ┌──────────────┐
        │   mod.rs     │  模块入口,导出公共 API
        └──────┬───────┘
               │ 依赖
    ┌──────────┼──────────┐
    │          │          │
    ▼          ▼          ▼
┌─────────┐ ┌────────┐ ┌──────────┐
│fd_table │ │path.rs │ │impls/    │
└────┬────┘ └───┬────┘ └────┬─────┘
     │          │           │
     │          │           │
     └──────────┼───────────┘
                │ 都依赖
                ▼
         ┌──────────────┐
         │   file.rs    │  File trait (会话层)
         │              │
         └──────┬───────┘
                │ RegFile 等持有
                ▼
         ┌──────────────┐
         │  dentry.rs   │  Dentry 结构
         │              │
         └──────┬───────┘
                │ 持有
                ▼
         ┌──────────────┐
         │  inode.rs    │  Inode trait (存储层)
         └──────────────┘

独立模块:
┌────────────┐  ┌──────────────┐  ┌────────────┐
│ mount.rs   │  │ file_lock.rs │  │  error.rs  │
│ (挂载表)   │  │ (文件锁)     │  │  (错误)    │
└────────────┘  └──────────────┘  └────────────┘

┌──────────────┐  ┌───────────────┐
│ file_system  │  │   dev/devno   │
│ (FS trait)   │  │  (设备管理)   │
└──────────────┘  └───────────────┘
```

### 依赖说明

1. **mod.rs → file.rs / path.rs / fd_table.rs**:模块入口导出所有公共 API,依赖各子模块
2. **fd_table.rs → file.rs**:FDTable 存储 `Arc<dyn File>`,依赖 File trait
3. **path.rs → dentry.rs / mount.rs**:路径解析需要 Dentry 和挂载表
4. **impls/* → file.rs / inode.rs**:RegFile 等实现 File trait,依赖 Dentry 和 Inode
5. **dentry.rs → inode.rs**:Dentry 持有 `Arc<dyn Inode>`,依赖 Inode trait
6. **file.rs ← inode.rs**:File trait 的 metadata() 返回 InodeMetadata,定义在 inode.rs
7. **所有模块 → error.rs**:统一的错误类型 FsError

### 关键数据流路径

#### 路径 1: open 系统调用

```
系统调用 sys_open(path, flags)
  → vfs_lookup(path)                     # path.rs
    → 解析路径组件,查找 Dentry          # 使用 DENTRY_CACHE
    → 检查挂载点                        # 使用 MOUNT_TABLE
    → 返回 Arc<Dentry>
  → RegFile::new(dentry, flags)          # impls/reg_file.rs
  → fd_table.alloc(Arc::new(file))       # fd_table.rs
  → 返回文件描述符 fd
```

#### 路径 2: read 系统调用

```
系统调用 sys_read(fd, buf, len)
  → fd_table.get(fd)                     # 获取 Arc<dyn File>
  → file.read(buf)                       # File trait 方法
    → (RegFile) inode.read_at(offset, buf)  # 委托给 Inode
      → (具体文件系统实现) 从磁盘读取数据
  → 更新 RegFile 内部的 offset
  → 返回读取字节数
```

#### 路径 3: mount 系统调用

```
系统调用 sys_mount(device, path, fs_type, flags)
  → 创建文件系统实例 fs: Arc<dyn FileSystem>
  → MOUNT_TABLE.mount(fs, path, flags, device)  # mount.rs
    → 创建 MountPoint,包含 fs 和 root Dentry
    → 添加到挂载表 mounts[path].push(mount_point)
    → 更新 DENTRY_CACHE 中的挂载点信息
  → 后续 vfs_lookup(path下的文件) 会自动切换到挂载的文件系统
```

#### 路径 4: 路径解析

```
vfs_lookup(\"/mnt/data/file.txt\")
  → parse_path() 解析为 [Root, \"mnt\", \"data\", \"file.txt\"]
  → 从根 Dentry 开始
  → resolve_component(\"mnt\")
    → base.lookup_child(\"mnt\")  # 先查 Dentry 缓存
    → 如果未命中: base.inode.lookup(\"mnt\")  # 查 Inode
    → 创建新 Dentry,加入缓存
    → check_mount_point()  # 检查是否为挂载点
  → resolve_component(\"data\") ...
  → resolve_component(\"file.txt\") ...
  → 返回最终 Dentry
```

## 核心机制

### 目录项缓存 (Dentry Cache)

Dentry 缓存是 VFS 性能的关键,避免了重复的路径解析和 Inode lookup。

#### 多级缓存结构

```
┌───────────────────────────────────────────────────────┐
│  DentryCache (全局缓存)                               │
│  ┌─────────────────────────────────────────────────┐  │
│  │  \"/\" → Weak<Dentry>                            │  │
│  │  \"/etc\" → Weak<Dentry>                        │  │
│  │  \"/etc/passwd\" → Weak<Dentry>                 │  │
│  │  \"/mnt/data\" → Weak<Dentry>                   │  │
│  └─────────────────────────────────────────────────┘  │
│                                                         │
│  使用 Weak<Dentry> 避免延长生命周期                   │
│  当 Dentry 不再被其他地方引用时自动从缓存消失          │
└───────────────────────────────────────────────────────┘

┌───────────────────────────────────────────────────────┐
│  Dentry 内部缓存 (父子关系)                           │
│  ┌─────────────────────────────────────────────────┐  │
│  │  Dentry(\"/etc\")                                │  │
│  │    parent: Weak<Dentry(\"/\")>                  │  │
│  │    children: {                                   │  │
│  │      \"passwd\" → Arc<Dentry>,                  │  │
│  │      \"hosts\" → Arc<Dentry>,                   │  │
│  │      ...                                         │  │
│  │    }                                             │  │
│  └─────────────────────────────────────────────────┘  │
│                                                         │
│  加速相对路径查找,避免每次都查询父 Inode               │
└───────────────────────────────────────────────────────┘
```

#### 缓存更新策略

- **插入时机**:路径解析成功后自动插入 `DENTRY_CACHE.insert(&dentry)`
- **失效时机**:Weak 引用自动失效,不需要手动清理
- **一致性保证**:文件删除时调用 `DENTRY_CACHE.remove(path)` 和 `parent.remove_child(name)`

### 挂载表 (Mount Table)

挂载表支持多文件系统共存,使用最长前缀匹配查找挂载点。

#### 挂载点栈

```
MOUNT_TABLE.mounts: BTreeMap<String, Vec<Arc<MountPoint>>>

例如:
{
  \"/\": [MountPoint(tmpfs, \"/\")],
  \"/mnt\": [
    MountPoint(fat32, \"/dev/sda1\"),   # 第一次挂载
    MountPoint(ext4, \"/dev/sda2\")     # 第二次挂载,覆盖
  ],
  \"/mnt/data\": [MountPoint(tmpfs, None)]
}

访问 \"/mnt/data/file\" 时:
1. 查找所有以 \"/mnt\" 开头的挂载点: [\"/\", \"/mnt\", \"/mnt/data\"]
2. 选择最长匹配: \"/mnt/data\"
3. 使用栈顶挂载点: MountPoint(tmpfs)
4. 从该挂载点的 root Dentry 开始解析剩余路径
```

#### 挂载点查找算法

```rust
// path.rs:check_mount_point()
fn check_mount_point(dentry: Arc<Dentry>) -> Result<Arc<Dentry>, FsError> {
    // 1. 快速路径:检查 dentry 本地缓存
    if let Some(mounted_root) = dentry.get_mount() {
        return Ok(mounted_root);
    }

    // 2. 慢速路径:查找挂载表
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

### 文件描述符表 (FDTable)

每个进程维护独立的文件描述符表,管理打开的文件。

#### FDTable 结构

```
FDTable {
  files: SpinLock<Vec<Option<Arc<dyn File>>>>,
  fd_flags: SpinLock<Vec<FdFlags>>,
  max_fds: usize
}

FD 分配策略:
- alloc() 总是返回最小的可用 FD
- install_at(fd) 可以指定 FD 编号 (用于 dup2)
- 数组动态扩展,最大 max_fds (通常 1024)

FD 标志 (fd_flags):
- FD_CLOEXEC: exec 时关闭该文件描述符
- 独立于文件状态标志 (O_RDONLY/O_WRONLY/O_APPEND 等)
```

#### dup 语义

```rust
// dup: 复制文件描述符,新旧 FD 指向同一个 Arc<dyn File>
let new_fd = fd_table.dup(old_fd)?;
// 共享偏移量: 新旧 FD 的 read/write 会相互影响 offset

// dup2: 复制到指定 FD,如果目标 FD 已打开则先关闭
let new_fd = fd_table.dup2(old_fd, target_fd)?;
// 特殊情况: old_fd == target_fd 时,直接返回,不关闭

// dup3: dup2 的扩展,支持设置 FD_CLOEXEC
let new_fd = fd_table.dup3(old_fd, target_fd, O_CLOEXEC)?;
// 不允许 old_fd == target_fd (返回 EINVAL)
```

### 引用计数与生命周期

VFS 使用 Rust 的智能指针管理对象生命周期,避免内存泄漏和悬空指针。

#### Dentry 引用关系

```
┌─────────────────────────────────────┐
│  Arc<Dentry(\"/etc\")>              │
│    ↑                                │
│    │ Arc (强引用)                   │
│    │                                │
│  ┌─┴─────────────────────┐          │
│  │ Dentry(\"/etc/passwd\")│          │
│  │   parent: Weak        │ ←──┐    │
│  │   inode: Arc          │    │    │
│  └───────────────────────┘    │    │
│                               │    │
│  Weak 避免循环引用:           │    │
│  父子互相引用会导致内存泄漏    │    │
└───────────────────────────────┼────┘
                                │
                                │ Arc (强引用)
                                ▼
                         ┌──────────────┐
                         │ Arc<Inode>   │
                         │ (可被多个    │
                         │  Dentry 共享)│
                         └──────────────┘
```

#### File 和 FDTable 的引用

```
Process {
  fd_table: Arc<FDTable>
}
  │
  │ Arc
  ▼
FDTable {
  files: Vec<Option<Arc<dyn File>>>
}
  │
  │ Arc
  ▼
RegFile {
  dentry: Arc<Dentry>,
  offset: AtomicUsize,
  flags: OpenFlags
}
  │
  │ Arc
  ▼
Dentry {
  inode: Arc<dyn Inode>
}

dup 后共享 File 对象:
fd[3] ──┐
        ├──> Arc<RegFile>
fd[4] ──┘

fork 后共享整个 FDTable:
Parent Process ──┐
                 ├──> Arc<FDTable>
Child Process  ──┘
```

## 设计决策

### 为什么分离 File 和 Inode?

**决策**:将文件抽象分为会话层 (File) 和存储层 (Inode) 两层。

**理由**:

1. **状态隔离**:同一个文件可被多次打开,每次有独立的状态 (offset、flags),但共享底层存储
2. **简化实现**:Inode 实现可以完全无状态,不需要考虑并发打开的 offset 管理
3. **支持硬链接**:多个 Dentry 可以共享同一个 Inode,符合 POSIX 语义
4. **管道等特殊文件**:PipeFile 不需要 Inode,直接实现 File trait,灵活性更高

**权衡**:增加了一层抽象,但换来了清晰的职责划分和更好的扩展性。

### 为什么使用 Dentry 缓存?

**决策**:维护全局 Dentry 缓存和 Dentry 内部的父子关系缓存。

**理由**:

1. **性能优化**:避免重复路径解析,减少 Inode lookup 操作 (磁盘 I/O)
2. **一致性**:所有路径解析返回相同的 Dentry 对象,简化状态管理
3. **减少内存**:Weak 引用允许不再使用的 Dentry 被自动回收

**权衡**:需要在文件删除/重命名时维护缓存一致性,但实际复杂度可控。

### 为什么挂载表使用最长前缀匹配?

**决策**:查找挂载点时使用最长前缀匹配算法,而不是精确匹配。

**理由**:

1. **层次化挂载**:支持 `/` 和 `/mnt` 同时作为挂载点,访问 `/mnt/file` 时自动使用 `/mnt`
2. **Linux 兼容**:Linux VFS 也使用最长前缀匹配
3. **灵活性**:可以在任意目录挂载新文件系统,无需特殊处理

**实现**:遍历所有挂载点,找到路径前缀最长的一个,时间复杂度 O(n),n 为挂载点数量 (通常很小)。

### 为什么支持挂载点栈?

**决策**:同一路径可以多次挂载,维护一个栈,最后挂载的文件系统覆盖之前的。

**理由**:

1. **容器支持**:容器技术需要在同一挂载点多次挂载 (mount namespace)
2. **调试方便**:可以临时挂载新文件系统,卸载后恢复原来的
3. **Linux 兼容**:Linux 支持 overmounting

**实现**:每个挂载路径对应一个 `Vec<Arc<MountPoint>>`,栈顶是当前可见的。

### 为什么 FDTable 使用 Vec 而不是 HashMap?

**决策**:FDTable 内部使用 `Vec<Option<Arc<dyn File>>>` 存储文件描述符。

**理由**:

1. **FD 编号连续**:POSIX 要求 alloc() 返回最小可用 FD,Vec 可以 O(n) 时间找到
2. **内存效率**:大部分进程只打开少量文件,Vec 更紧凑
3. **缓存友好**:Vec 的内存布局连续,访问 FD 时缓存命中率高

**权衡**:如果进程打开大量文件且稀疏分布,Vec 可能浪费空间,但实际场景很少见。

### 为什么使用 trait 对象而不是枚举?

**决策**:File 和 Inode 使用 trait 对象 (`Arc<dyn File>`),而不是枚举 (`enum File { Reg, Pipe, ... }`)。

**理由**:

1. **扩展性**:可以在外部 crate 中添加新的文件类型,无需修改 VFS 核心代码
2. **代码复用**:不同文件类型共享相同的操作接口,FDTable 等无需关心具体类型
3. **动态分发**:支持运行时多态,灵活性更高

**权衡**:trait 对象有轻微的虚函数调用开销,但在 VFS 场景下可以忽略 (I/O 开销远大于调用开销)。

## 性能考量

### 关键优化

1. **Dentry 缓存**:避免重复路径解析,减少 Inode lookup (磁盘 I/O) - 这是最重要的性能优化
2. **挂载点缓存**:Dentry 本地缓存挂载点信息,避免每次查挂载表
3. **父子关系缓存**:Dentry 内部缓存子项,加速相对路径查找
4. **Weak 引用**:全局缓存使用 Weak,不延长 Dentry 生命周期,减少内存占用
5. **原子操作**:RegFile 的 offset 使用 AtomicUsize,避免锁开销

### 性能瓶颈

1. **路径解析**:深层路径需要多次 Inode lookup,即使有缓存第一次访问仍然慢
   - **建议**:尽量使用绝对路径,避免 `../../..` 等复杂相对路径
2. **挂载点查找**:O(n) 时间复杂度,如果挂载点很多可能变慢
   - **建议**:限制挂载点数量,或使用前缀树优化 (未实现)
3. **全局 Dentry 缓存锁**:并发查找时可能竞争 SpinLock
   - **建议**:未来可考虑分片锁或无锁缓存

### 预期性能

在典型的 RISC-V 平台上:

- **vfs_lookup 缓存命中**:约 100-500 纳秒 (查 BTreeMap + 克隆 Arc)
- **vfs_lookup 缓存未命中**:约 10-100 微秒 (Inode lookup + 创建 Dentry)
- **read/write 系统调用**:约 1-10 微秒 (不含实际 I/O)
- **open 系统调用**:约 5-50 微秒 (路径解析 + 创建 File 对象)

**注意**:实际性能取决于底层文件系统实现、磁盘速度、编译器优化级别等。

## 安全性分析

### 安全机制

1. **类型安全**:Rust 类型系统保证内存安全,无空指针、无数据竞争
2. **引用计数**:Arc/Weak 自动管理生命周期,无手动 free,避免 use-after-free
3. **权限检查**:FileMode 提供权限位检查 (当前简化为 root-only,未来支持多用户)
4. **路径规范化**:normalize_path 防止 `../../../` 越过根目录
5. **挂载隔离**:进程可以有独立的挂载命名空间 (未实现,规划中)

### 已知限制

1. **权限系统简化**:当前假设所有操作都是 root 用户,权限检查未完全实现
   - **影响**:无法防止恶意进程访问其他用户文件
2. **符号链接循环**:vfs_lookup 不检测符号链接循环,可能导致栈溢出
   - **影响**:恶意构造的符号链接可能导致内核崩溃
3. **缓存一致性**:目录删除后,Dentry 缓存可能残留
   - **影响**:可能访问到已删除的文件,需要手动调用 remove 清理
4. **挂载点安全**:未限制挂载操作的权限
   - **影响**:任何进程都可以随意挂载文件系统

### 未来改进

1. **完整权限系统**:实现 uid/gid 检查,支持多用户
2. **符号链接限制**:限制解析深度 (如 Linux 的 40 层),检测循环
3. **mount namespace**:支持进程级挂载命名空间,隔离容器
4. **capability**:细粒度权限控制,如 CAP_SYS_ADMIN 控制挂载权限

## 扩展可能性

未来可能的扩展方向:

1. **并发优化**:无锁 Dentry 缓存,减少锁竞争
2. **网络文件系统**:支持 NFS、9P 等远程文件系统协议
3. **文件系统堆栈**:支持 overlayfs、unionfs 等组合文件系统
4. **异步 I/O**:支持 io_uring 风格的异步文件操作
5. **内存映射文件**:实现 mmap 系统调用,支持文件映射到进程地址空间
6. **文件系统快照**:支持 COW 文件系统 (如 btrfs、zfs)
7. **实时监控**:inotify/fanotify 风格的文件系统事件通知

这些扩展在不破坏现有 API 的前提下都是可行的,得益于分层架构的良好封装。
