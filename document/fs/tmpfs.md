# Tmpfs

Tmpfs 是内存文件系统, 当前主要用于 `/tmp` 等临时路径. 它实现 VFS `FileSystem` 和 `Inode`, 不依赖块设备.

## 当前状态

- 源码位于 `os/src/fs/tmpfs/`.
- `TmpFs` 保存根 inode 和全局统计.
- `TmpfsInode` 表示目录, 文件和 symlink.
- 容量限制以页为单位统计, `max_size_mb = 0` 表示无限制.
- `mount_tmpfs` 是 FS 初始化阶段和其他挂载路径使用的便捷入口.

## 目标

- 为临时文件提供快速读写路径.
- 支持目录, 普通文件, symlink 和基础元数据.
- 在 rootfs 之上挂载 `/tmp`, 避免临时文件污染持久化 rootfs.
- 为测试提供不依赖磁盘镜像的可写文件系统.

## 非目标

- 不提供 swap 后端.
- 不实现 Linux tmpfs 的所有 mount options.
- 不在文档中列出每个 inode 操作分支.

## 模块边界

- `tmpfs.rs`: 文件系统实例, 容量统计, statfs.
- `inode.rs`: 内存 inode, 数据页, 目录项和 symlink 内容.
- `fs/mod.rs`: `mount_tmpfs` 初始化入口.

## 关键流程

### mount

```text
TmpFs new
  -> root TmpfsInode
  -> MOUNT_TABLE mount
  -> path lookup enters tmpfs
```

### write

普通文件写入会按页扩展内存数据, 并更新统计. 如果设置了容量上限, 分配前需要检查剩余页数.

### umount

tmpfs 没有持久化同步. 当 mount table 和 open file 都释放对应 `Arc`, 内存由 Rust 生命周期释放.

## 并发和生命周期约束

- 文件系统统计和 inode 内容由锁保护.
- open file 的 offset 仍属于 VFS `File`, tmpfs inode 只保存内容和元数据.
- 容量统计必须和 truncate/unlink 等释放路径保持一致.

## 已知限制

- 无 swap 和回收策略.
- 无限容量配置仍受实际内核内存限制.
- 权限和时间戳语义以当前 VFS 需要为主, 不是完整 Linux tmpfs.

## 源码索引

- `os/src/fs/tmpfs/tmpfs.rs`: `TmpFs`, 容量和 statfs.
- `os/src/fs/tmpfs/inode.rs`: `TmpfsInode`.
- `os/src/fs/mod.rs`: `mount_tmpfs`.
- `os/src/fs/tests/tmpfs/`: tmpfs 行为测试.
