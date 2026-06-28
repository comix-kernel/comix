# VFS 协作流程

本页不是 API 使用手册, 而是说明系统调用, VFS, FS 和设备层如何协作. 具体调用方式请读 rustdoc 和对应 syscall 实现.

## 当前状态

VFS 已覆盖常见文件系统调用所需的核心路径:

- path lookup, open, read, write, lseek, close.
- mkdir, unlink, rmdir, rename, link, symlink, readlink.
- getdents/stat/statfs/chmod/chown/utimens 类元数据操作.
- pipe, stdio, 设备文件, 文件锁.
- mount/umount 的基础命名空间能力.

## 目标

- 帮助贡献者判断改动应该放在 syscall, VFS, FS 还是 device 层.
- 避免把具体实现细节复制进正式文档.
- 明确常见路径的状态归属和生命周期.

## 非目标

- 不提供完整代码示例集合.
- 不替代系统调用文档.
- 不承诺每个 POSIX corner case 都已实现.

## 常见改动定位

- 修改 fd 分配, dup, close-on-exec: `fd_table.rs`.
- 修改普通文件 offset 或 append 语义: `impls/reg_file.rs`.
- 修改路径解析, symlink, cwd/root 行为: `path.rs`.
- 修改 mount/umount: `mount.rs` 和 syscall 层.
- 修改文件系统内部目录或数据行为: 对应 `os/src/fs/<name>/`.
- 修改块设备读写或分区: `os/src/device/block/`.
- 修改 `/dev` 节点或设备号: `fs/mod.rs`, `vfs/devno.rs`, device driver.

## 关键流程

### 用户路径到 fd

```text
syscall args
  -> normalize and lookup path
  -> dentry and inode type
  -> file implementation
  -> fd table slot
```

判断 bug 时先确认问题发生在路径命名空间, file 会话, inode 操作, 还是 fd table slot.

### fd 到数据

```text
fd
  -> Arc dyn File
  -> file-specific state
  -> inode or driver
```

如果两个 fd 来自 `dup`, 它们共享 file-specific state. 如果来自两次 open, 它们通常共享 inode, 但不共享普通文件 offset.

### mount 后访问

```text
mount fs at /mnt
  -> /mnt dentry points to mounted root
  -> /mnt/a lookup enters mounted fs
```

调用者不需要在每次访问时指定 fs type. fs type 只在 mount 创建文件系统实例时有意义.

### rootfs 初始化后访问设备

```text
discover block devices
  -> pick ext4 rootfs containing /bin/sh or /bin/ash
  -> create /dev
  -> expose vda and partitions
  -> mount procfs sysfs tmpfs
```

VFAT 分区当前用于 mount/umount 和 FAT 兼容路径测试, 不作为默认 rootfs.

## 并发和生命周期约束

- 不要把 fd 生命周期和 inode 生命周期混为一谈. fd close 只释放 fd slot 引用.
- 不要让具体 FS 保存 VFS 的强 dentry 引用形成环.
- 不要在持有全局 mount/dentry 锁时做长时间块设备 I/O.
- 动态文件系统默认应谨慎使用缓存, 尤其是基于进程生命周期的路径.

## 已知限制

- 文档中的流程是当前设计目标, 具体 syscall 的兼容性仍以测试和源码为准.
- mount flags, permission checks, namespace 隔离仍有不完整处.
- 设备热插拔路径尚未形成自动 `/dev` 更新机制.

## 源码索引

- `os/src/vfs/`: VFS 核心.
- `os/src/fs/`: 具体文件系统实现和初始化.
- `os/src/device/`: 设备驱动和块设备分区.
- `os/src/vfs/tests/`: VFS 行为测试.
- `os/src/fs/tests/`: FS 行为测试.
- `os/src/device/tests/`: 块设备和分区测试.
