# VFS 架构

VFS 的核心设计是把"名字", "打开会话", "存储对象", "挂载边界"分开. 系统调用只处理 fd 和路径, 具体文件系统只处理 inode 和数据, 设备层只暴露驱动接口.

## 当前状态

当前实现已经形成稳定的五个层次:

```text
syscall and task state
  -> FDTable
  -> File
  -> Dentry tree and MountTable
  -> Inode
  -> FileSystem and concrete FS
```

- `FDTable` 是每个任务可见的 fd 命名空间.
- `File` 是打开文件描述, 可以是普通文件, 管道, stdio, 字符设备或块设备.
- `Dentry` 是路径缓存节点, 保存名字, 父子关系, inode 和 mount 关系.
- `Inode` 是底层文件对象, 由 ext4, tmpfs, procfs, sysfs, VFAT 等实现.
- `FileSystem` 是一次挂载的文件系统实例.

## 目标

- 清晰分离 open-time 状态和 storage-time 状态.
- 支持同一路径树中混合多个文件系统.
- 支持 `dup` 共享 open file description 的语义.
- 为动态伪文件系统和设备节点保留扩展点.

## 非目标

- 不复制 Linux VFS 的所有缓存, permission 和 superblock 细节.
- 不在架构文档中列出每个 trait 方法.
- 不把 ext4, tmpfs, VFAT 的内部实现放进 VFS 架构文档.

## 分层职责

### FDTable

`FDTable` 保存 fd 到 `Arc<dyn File>` 的映射. fd 是进程视角的整数索引, `File` 是被打开的对象. `dup` 和 `fork` 可以共享同一个 `File`, 因此共享 offset 和 file status flags. fd flags 仍属于 fd slot.

### File

`File` 表达一次打开会话. 普通文件会保存 offset 并转发到 inode 的随机访问接口. 管道是流式对象, 没有 seek 语义. 设备文件根据 inode 中的设备号进入设备驱动.

### Dentry and MountTable

`Dentry` 把路径组件映射到 inode, 并缓存父子关系. `MountTable` 把路径映射到挂载点栈. 路径解析命中挂载点后切换到挂载文件系统的根 dentry, 调用者不需要知道跨越了哪个文件系统.

### Inode

`Inode` 是文件系统对象接口. 它不保存打开会话 offset, 因此可以被多个 `File` 共享. 目录 lookup, create, unlink, rename 和 readdir 也在这个层次完成.

### FileSystem

`FileSystem` 表达一次具体文件系统挂载. VFS 只需要根 inode, sync, statfs 和 umount 等挂载级操作. 块设备适配, 内存数据结构或动态生成逻辑由具体 FS 自己负责.

## 关键流程

### 打开普通文件

```text
sys_open
  -> vfs_lookup
  -> Dentry
  -> RegFile
  -> FDTable alloc
```

路径解析只返回命名空间对象. 打开时才创建 `File`, 这让同一个 dentry 可以被多次打开并拥有不同 offset.

### 读取普通文件

```text
sys_read
  -> FDTable get
  -> File read
  -> Inode read_at
  -> concrete FS
```

`File` 负责会话状态, `Inode` 负责真实数据. 这个分界是 VFS 最重要的生命周期边界.

### 挂载文件系统

```text
FileSystem root_inode
  -> MountPoint root Dentry
  -> MountTable push
  -> path lookup crosses mount point
```

同一个 mount path 可以重复挂载. 栈顶是当前可见挂载, umount 后恢复下层挂载.

### 根文件系统探测

当前启动路径由 FS 层驱动, VFS 只提供 mount 和 lookup 能力. `init_rootfs_from_discovered_block_devices` 会遍历块设备和分区, 尝试打开 ext4, 并选择包含 `/bin/sh` 或 `/bin/ash` 的设备作为 `/`.

## 并发和生命周期约束

- VFS 对象广泛使用 `Arc` 共享生命周期, 使用 `Weak` 打断反向引用.
- `Dentry` 的 parent 和 global cache 不应持有强引用到上游对象.
- 挂载表和 dentry cache 的锁粒度较粗, 适合当前内核规模, 不是高度并行文件服务器设计.
- `File` 内部是否需要原子 offset 或锁由具体实现决定, 但 trait 要求 `Send + Sync`.
- procfs 动态路径要谨慎缓存, 当前通过 `cacheable` 让实现决定是否进入 dentry cache.

## 已知限制

- permission, namespace, chroot 等策略尚未形成完整安全模型.
- mount propagation, bind mount, lazy umount 等 Linux 高级语义未实现.
- dentry 缓存没有完整的统一 invalidation 协议.
- `FileSystem` 没有完整 superblock 概念, 挂载状态较轻量.

## 源码索引

- `os/src/vfs/mod.rs`: 模块入口和设计性 rustdoc.
- `os/src/vfs/fd_table.rs`: fd slot 生命周期, dup, close-on-exec.
- `os/src/vfs/file.rs`: 打开文件会话边界.
- `os/src/vfs/impls/reg_file.rs`: 普通文件如何桥接 File 和 Inode.
- `os/src/vfs/impls/pipe_file.rs`: 流式文件模型.
- `os/src/vfs/dentry.rs`: dentry 树, weak cache, mount metadata.
- `os/src/vfs/path.rs`: lookup 状态机.
- `os/src/vfs/mount.rs`: mount stack 和 root mount.
- `os/src/vfs/inode.rs`: 存储对象接口.
- `os/src/vfs/file_system.rs`: 挂载级文件系统接口.
