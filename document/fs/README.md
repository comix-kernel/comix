# FS 文件系统实现层

FS 层提供 Comix 当前可挂载的具体文件系统. VFS 负责路径, fd 和挂载表; FS 负责实现 `FileSystem` 和 `Inode`, 并把内存结构, 块设备或动态内核状态映射成文件树.

本文档只同步设计和边界. 具体 API, 字段和错误分支以 `os/src/fs/` 源码和 rustdoc 为准.

## 当前状态

| 文件系统 | 源码路径 | 主要用途 | 存储来源 |
| --- | --- | --- | --- |
| ext4 | `os/src/fs/ext4/` | 默认 rootfs 候选, 持久化读写 | 块设备或分区 |
| VFAT/FAT | `os/src/fs/vfat/` | FAT/VFAT mount 兼容路径, mount/umount 测试分区 | 块设备或分区 |
| tmpfs | `os/src/fs/tmpfs/` | `/tmp` 等临时目录 | 内存页 |
| procfs | `os/src/fs/proc/` | `/proc` 进程和系统快照 | 动态生成 |
| sysfs | `os/src/fs/sysfs/` | `/sys` 设备和内核属性 | 设备注册表 |
| simple_fs | `os/src/fs/simple_fs.rs` | rootfs fallback 和测试镜像 | 编译期嵌入 ramdisk |

注意: 当前 FAT 相关源码目录是 `os/src/fs/vfat/`, 文档和索引不应再使用过期 FAT 目录名.

## 目标

- 为 VFS 提供多个可互换的文件系统实现.
- 让 ext4 rootfs 自动从已发现块设备和分区中选择.
- 让 procfs/sysfs/tmpfs 在 rootfs 之上提供运行时文件树.
- 让 VFAT/FAT 分区用于 mount/umount 和兼容性测试, 不作为默认 rootfs.

## 非目标

- 不在 FS 文档中维护完整 trait 实现清单.
- 不复述第三方库内部结构.
- 不承诺每个 Linux 文件系统高级特性均实现.

## 初始化和 rootfs 探测

当前默认路径是分区盘探测:

```text
device discovery
  -> list block disks and partitions
  -> sort partitions before whole disks
  -> try ext4 open
  -> mount temporarily at /
  -> accept only if /bin/sh or /bin/ash exists
  -> create common mount dirs
  -> mount procfs, sysfs, tmpfs and create /dev nodes
```

默认运行镜像预期是分区盘: `vda1` 一般承载 ext4 rootfs, `vda2` 预留给 VFAT/FAT mount/umount 测试. 代码不依赖固定顺序, 而是按内容探测 rootfs. 如果无法找到含 shell 的 ext4 rootfs, 才回退到编译期嵌入的 simple_fs.

rootfs 选中后会确保 `/dev`, `/proc`, `/sys`, `/tmp`, `/mnt`, `/tests` 等顶层挂载点存在. `/dev` 节点随后根据设备注册表创建, 包括整盘和分区块设备.

## 模块边界

- `fs/mod.rs`: 文件系统初始化, rootfs 探测, tmpfs/procfs/sysfs 挂载, `/dev` 节点创建.
- `ext4/`: ext4_rs 适配, root inode 和 ext4 inode 操作.
- `vfat/`: fatfs 适配, VFAT/FAT 文件树接入 VFS.
- `tmpfs/`: 内存页和 inode 统计.
- `proc/`: 动态 generator 和进程路径.
- `sysfs/`: 设备注册表到 `/sys` 的冷插拔树.
- `simple_fs.rs`: 嵌入式只读镜像和 fallback rootfs.
- `smfs.rs`: 简单内存文件系统实验路径.

## 并发和生命周期约束

- 所有 FS 实例通过 `Arc<dyn FileSystem>` 进入 mount table.
- inode 生命周期通常由 dentry 和 open file 间接持有.
- 块设备 FS 必须把底层驱动错误转换成 `FsError`.
- rootfs probe 会临时挂载候选 ext4, 不符合条件时卸载并清空当前任务 root/cwd 与 dentry cache.
- VFAT 当前串行化对底层 `fatfs` 的访问, 以匹配库的打开和 unmount 模型.

## 已知限制

- rootfs 只从 ext4 候选中选择, VFAT 分区用于测试和兼容挂载.
- ext4 不覆盖完整 journaling 和全部 Linux 高级特性.
- procfs/sysfs 当前以只读和冷插拔为主.
- simple_fs 是只读 fallback, 不是生产 rootfs 格式.

## 文档导航

- [ext4.md](ext4.md): ext4 rootfs 和块设备适配.
- [vfat.md](vfat.md): VFAT/FAT mount 兼容路径.
- [tmpfs.md](tmpfs.md): 内存临时文件系统.
- [procfs.md](procfs.md): `/proc` 动态文件树.
- [sysfs.md](sysfs.md): `/sys` 设备视图.
- [simple_fs.md](simple_fs.md): 编译期嵌入 fallback 文件系统.

## 源码索引

- `os/src/fs/mod.rs`: FS 初始化和 rootfs 探测入口.
- `os/src/fs/ext4/`: ext4 implementation.
- `os/src/fs/vfat/`: VFAT/FAT implementation.
- `os/src/fs/tmpfs/`: tmpfs implementation.
- `os/src/fs/proc/`: procfs implementation.
- `os/src/fs/sysfs/`: sysfs implementation and device registry.
- `os/src/fs/simple_fs.rs`: simple fs fallback.
- `os/src/device/block/partition.rs`: MBR/GPT 分区块设备.
- `os/src/vfs/mount.rs`: 挂载表消费 FS 实例.
