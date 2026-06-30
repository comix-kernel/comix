# FileSystem 与错误策略

`FileSystem` 是 VFS 对一次挂载文件系统实例的最小抽象. 具体文件系统通过它暴露根 inode, 同步, statfs 和卸载语义. 具体文件和目录行为主要在 `Inode` 层.

## 当前状态

- ext4, tmpfs, procfs, sysfs, simple_fs, VFAT 都接入 VFS.
- `FileSystem::fs_type` 用于 mount 列表和 procfs 展示.
- `FileSystem::root_inode` 是挂载接入点.
- `FsError` 统一 VFS 和文件系统错误, 并映射为 Linux errno.
- VFAT 当前源码路径是 `os/src/fs/vfat/`, VFS 文档只引用这个入口.

## 目标

- 让每个文件系统只暴露 VFS 需要的挂载级能力.
- 保持错误类型跨 VFS, FS, 设备适配层可转换.
- 让系统调用层可以稳定把 `FsError` 转成 errno.
- 避免在正式文档中复制每个错误分支.

## 非目标

- 不维护一个完整错误码大全. 以 `os/src/vfs/error.rs` 为准.
- 不规定每个具体 FS 的全部内部错误.
- 不在本页提供自定义文件系统教程式代码.

## 模块边界

- `file_system.rs`: 挂载实例接口和 statfs 结构.
- `error.rs`: VFS 统一错误和 errno 映射.
- `mount.rs`: 消费 `FileSystem` 并创建 `MountPoint`.
- 具体 FS: 负责把内部库或设备错误转换成 `FsError`.

## 关键流程

### mount integration

```text
concrete FS open
  -> Arc dyn FileSystem
  -> root_inode
  -> MountPoint
  -> VFS namespace
```

root inode 一旦进入 mount table, 后续路径解析只和 `Dentry`/`Inode` 交互. `FileSystem` 主要服务于挂载级操作.

### error conversion

```text
device or fs error
  -> FsError
  -> syscall errno
```

ext4 和 VFAT 都有内部库错误. 文档只描述转换边界, 具体映射由源码保存, 避免文档和实现漂移.

### statfs and sync

`statfs` 汇总文件系统容量和能力信息. `sync` 是挂载级同步入口, 对内存或动态文件系统可能是空操作, 对块设备文件系统会下推到设备 flush 或库 unmount/sync 路径.

## 并发和生命周期约束

- `FileSystem` 必须 `Send + Sync`, 因为 mount table 全局共享.
- 是否需要内部大锁由具体 FS 决定. VFAT 由于底层 `fatfs` 访问模型, 当前使用挂载状态锁串行化操作.
- `umount` 不能破坏仍被 `Arc` 持有的 open file. 当前 VFS 的 umount 语义较轻量, 调用方要避免卸载忙碌文件系统.
- 错误类型不要携带短生命周期引用, 需要能跨层返回.

## 已知限制

- busy mount 检测不完整.
- mount flags 对错误策略的影响尚不完整.
- `FsError` 是内核内部通用错误, 不能表达每个底层库的所有细节.
- statfs 字段对伪文件系统和 FAT/VFAT 这类无 inode 计数的文件系统会使用近似或 0.

## 源码索引

- `os/src/vfs/file_system.rs`: `FileSystem` 和 `StatFs`.
- `os/src/vfs/error.rs`: `FsError` 和 errno 映射.
- `os/src/vfs/mount.rs`: 挂载表如何消费文件系统实例.
- `os/src/fs/ext4/mod.rs`, `os/src/fs/ext4/inode.rs`: ext4 接入.
- `os/src/fs/tmpfs/tmpfs.rs`, `os/src/fs/tmpfs/inode.rs`: tmpfs 接入.
- `os/src/fs/proc/proc.rs`, `os/src/fs/proc/inode.rs`: procfs 接入.
- `os/src/fs/sysfs/sysfs.rs`, `os/src/fs/sysfs/inode.rs`: sysfs 接入.
- `os/src/fs/vfat/fs.rs`, `os/src/fs/vfat/inode.rs`, `os/src/fs/vfat/adapter.rs`: VFAT/FAT 接入.
