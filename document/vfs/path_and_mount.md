# 路径解析与挂载

路径解析把用户传入的字符串转换为 `Dentry`. 挂载管理把多个文件系统拼接到同一个路径树中. 这两个机制共同决定命名空间中"看到的文件"来自哪里.

## 当前状态

- 支持绝对路径和相对路径.
- 支持 `.`, `..`, symlink 跟随和 no-follow 变体.
- 路径解析过程中会检查 dentry 本地 mount cache 和全局 `MountTable`.
- `MountTable` 对同一路径保存挂载点栈, 栈顶可见.
- 根文件系统由 FS 初始化代码探测后挂载到 `/`.

## 目标

- 让路径查找对调用者隐藏文件系统边界.
- 让 mount/umount 只更新挂载表和 dentry mount cache.
- 让 lookup 可以在动态文件系统中选择不缓存.
- 支持 probe rootfs 时临时挂载和回滚.

## 非目标

- 不实现 Linux mount namespace, bind mount, propagation.
- 不在本文维护 path parser 的完整分支.
- 不保证所有 mount flags 已经执行到每个访问路径.

## 模块边界

- `path.rs`: 路径规范化, split, lookup, symlink, mount crossing.
- `mount.rs`: mount point, mount stack, root dentry, probe umount.
- `dentry.rs`: mount point cache 和 mounted-on 反向关系.
- FS 层负责决定挂载哪个文件系统, VFS 只负责把它接入路径树.

## 关键流程

### 普通 lookup

```text
path string
  -> start dentry
  -> component loop
  -> child cache or inode lookup
  -> optional symlink follow
  -> mount point check
  -> final dentry
```

`vfs_lookup` 默认跟随最终 symlink. `vfs_lookup_no_follow` 用于 unlink, lstat 等需要操作 link 本身的路径.

### mount crossing

```text
/mnt dentry
  -> get_mount hit
  -> mounted root dentry
  -> continue lookup inside mounted fs
```

如果 dentry 还没有本地 mount cache, `path.rs` 会查询 `MountTable`, 命中后回填 dentry 以加速后续解析.

### umount

umount 从指定路径的挂载栈弹出栈顶. 如果下面还有挂载, dentry 指向下层根. 如果没有, 清除挂载标记. 根挂载不允许普通 umount, rootfs probe 使用专门路径回滚临时根.

### rootfs probe

`init_rootfs_from_discovered_block_devices` 遍历 sysfs 设备注册表看到的块设备和分区, 优先分区设备. 每个候选设备会被临时作为 ext4 挂载到 `/`, 然后检查 `/bin/sh` 或 `/bin/ash`. 不符合条件的候选会卸载并清空当前任务 root/cwd 和 dentry cache.

## 并发和生命周期约束

- `MountTable` 由锁保护, mount/umount 是全局命名空间操作.
- 挂载点根 dentry 持有 root inode, 其 mounted-on 关系用 weak 指向外层 dentry.
- dentry 的 mount cache 是加速路径, 真正来源仍是 mount table.
- path lookup 过程中不要长期持有不必要的锁后调用文件系统实现, 否则容易放大锁竞争.

## 已知限制

- mount flags 如 read-only, noexec, nodev 的执行仍不完整.
- 没有 per-task mount namespace.
- dentry cache 清理较粗, rootfs probe 直接清空全局缓存.
- symlink 解析有递归深度限制, 具体限制以源码为准.

## 源码索引

- `os/src/vfs/path.rs`: lookup 主流程, symlink, mount check.
- `os/src/vfs/mount.rs`: mount stack, root mount, probe rollback.
- `os/src/vfs/dentry.rs`: mount cache, mounted-on, full path.
- `os/src/fs/mod.rs`: rootfs probe 和初始化挂载顺序.
- `os/src/fs/sysfs/device_registry.rs`: 块设备和分区枚举来源.
