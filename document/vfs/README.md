# VFS 子系统

VFS 是 Comix 内核的统一文件命名空间和打开文件模型. 它把系统调用看到的路径, 文件描述符, 设备文件和具体文件系统实现分开, 让 ext4, tmpfs, procfs, sysfs, VFAT 以及设备节点都能通过同一组抽象接入.

本文档只描述设计边界和关键生命周期. 具体 trait 方法, 字段和错误分支以 `os/src/vfs/` rustdoc 和源码为准.

## 当前状态

- `File` 是打开文件会话, 保存 offset, open flags 和具体读写语义.
- `Inode` 是文件系统对象, 提供无状态的随机访问, 目录操作和元数据.
- `Dentry` 是路径层缓存节点, 连接名字, 父子关系和 `Inode`.
- `MountTable` 维护全局挂载点栈, 路径解析会自动跨越挂载边界.
- `FDTable` 是进程可见的 fd 空间, fd 指向 `Arc<dyn File>`.
- `dev` 和 `devno` 把 POSIX 设备号映射到字符/块设备驱动.

## 目标

- 为系统调用提供稳定的 POSIX 风格文件访问模型.
- 允许不同文件系统只实现 `FileSystem` 和 `Inode`, 不关心 fd 分配和路径缓存.
- 允许同一个 `Inode` 被多个 `File` 会话共享, 同时保持每次 open 的 offset 独立.
- 允许设备节点作为普通路径出现, 由 VFS 根据 inode 类型和设备号转发到驱动.

## 非目标

- 不在正式文档中维护完整 API 清单.
- 不把某个文件系统的内部格式写入 VFS 文档.
- 不承诺完整 Linux VFS 兼容性. 当前实现以 Comix 需要的系统调用语义为准.

## 模块边界

```text
syscall layer
  -> FDTable
  -> File
  -> Dentry and path
  -> Inode
  -> FileSystem implementation
  -> device layer when backed by hardware
```

- `file.rs`: 打开文件会话接口.
- `inode.rs`: 文件系统对象接口和基础元数据类型.
- `dentry.rs`: 路径节点, 父子缓存, 挂载点反向关系.
- `path.rs`: 绝对/相对路径解析, symlink 跟随, mount crossing.
- `mount.rs`: 全局挂载表, 根挂载, probe 期间根卸载.
- `fd_table.rs`: fd 分配, dup, close, close-on-exec.
- `file_system.rs`: 文件系统实例接口.
- `file_lock.rs`: 全局 advisory 文件锁.
- `dev.rs`, `devno.rs`, `impls/*_dev_file.rs`: 设备号和设备文件.

## 关键流程

### open

1. 系统调用层解析 flags 和 mode.
2. `path.rs` 把路径解析为 `Dentry`.
3. 根据 inode 类型创建 `RegFile`, `CharDeviceFile`, `BlockDeviceFile` 或其他 `File` 实现.
4. 当前任务的 `FDTable` 分配最小可用 fd.

### read and write

1. 系统调用层通过 fd 取出 `Arc<dyn File>`.
2. `File` 根据自身类型处理 offset, flags 和流式语义.
3. 普通文件转发到 `Inode::read_at` 或 `write_at`.
4. 设备文件通过设备号转发到字符或块设备驱动.

### path lookup

1. 绝对路径从当前任务 root 或全局 root 开始, 相对路径从 cwd 开始.
2. 每个组件先查 `Dentry` 子缓存, miss 后调用父 inode 的 lookup.
3. 成功 lookup 后创建子 `Dentry`, 视 inode 的 `cacheable` 策略加入缓存.
4. 每步都会检查 mount point, 命中时切换到挂载文件系统根 dentry.

### mount

1. `FileSystem::root_inode` 生成被挂载文件系统根 inode.
2. `MountTable` 为挂载点创建根 `Dentry` 和 `MountPoint`.
3. 同一路径允许形成栈, 栈顶为当前可见文件系统.
4. 卸载时恢复下层挂载或清除挂载缓存.

## 并发和生命周期

- `FDTable`, `Dentry` 子节点, `DENTRY_CACHE`, `MountTable`, 文件锁表使用内核锁保护.
- `File` 和 `Inode` 以 trait object + `Arc` 共享. `dup` 共享同一个 `File`, 因此也共享 offset.
- `Dentry` 到父节点和全局缓存使用 `Weak`, 避免父子环和缓存永久持有对象.
- procfs 这类动态路径可以通过 `Inode::cacheable` 禁止 dentry 缓存, 避免进程退出后的陈旧路径.
- mount root 记录 mounted-on 关系, 使 full path 和 `..` 能跨挂载边界工作.

## 已知限制

- mount flags 主要作为结构化状态保存, 并非所有 flags 都完整强制执行.
- dentry 失效仍较粗粒度, 跨文件系统 rename/unlink 后的一致性依赖各实现配合.
- 文件锁是 advisory lock, 不会自动阻止未遵守锁协议的读写路径.
- 设备文件覆盖的是当前内核已有驱动集合, 不是完整 Linux devtmpfs.

## 文档导航

- [architecture.md](architecture.md): VFS 分层和生命周期总览.
- [inode_and_dentry.md](inode_and_dentry.md): 存储对象和路径缓存设计.
- [file_and_fdtable.md](file_and_fdtable.md): 打开文件会话和 fd 语义.
- [path_and_mount.md](path_and_mount.md): 路径解析和挂载表.
- [filesystem_and_errors.md](filesystem_and_errors.md): 文件系统接入边界和错误策略.
- [filelock_and_devices.md](filelock_and_devices.md): 文件锁和设备节点.
- [usage.md](usage.md): 子系统协作流程.

## 源码索引

- `os/src/vfs/mod.rs`: VFS 模块入口和公共导出.
- `os/src/vfs/file.rs`: `File` 会话层接口.
- `os/src/vfs/inode.rs`: `Inode` 存储层接口和元数据.
- `os/src/vfs/dentry.rs`: dentry 结构, 缓存和挂载关系.
- `os/src/vfs/path.rs`: 路径解析, symlink, mount crossing.
- `os/src/vfs/mount.rs`: `MountTable`, `MountPoint`, 根挂载.
- `os/src/vfs/fd_table.rs`: fd table 和 dup/exec 生命周期.
- `os/src/vfs/file_lock.rs`: advisory lock 管理.
- `os/src/vfs/dev.rs`, `os/src/vfs/devno.rs`: 设备号和驱动注册表.
- `os/src/vfs/impls/`: 普通文件, 管道, stdio, 字符设备, 块设备文件实现.
