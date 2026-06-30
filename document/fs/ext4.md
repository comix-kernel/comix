# Ext4

Ext4 是当前默认 rootfs 的持久化文件系统候选. FS 初始化会在已发现块设备和分区中寻找可打开的 ext4, 并以 `/bin/sh` 或 `/bin/ash` 判断它是否是可启动 rootfs.

## 当前状态

- 源码位于 `os/src/fs/ext4/`.
- 使用 `ext4_rs` 作为底层 ext4 操作库.
- `BlockDeviceAdapter` 把内核 `BlockDriver` 适配成 ext4_rs 可读写的块设备.
- `Ext4FileSystem` 实现挂载级 `FileSystem`.
- `Ext4Inode` 实现 VFS `Inode`, 并通过 weak dentry 反向引用按需取得路径.

## 目标

- 作为默认 rootfs 的主要格式.
- 支持普通文件, 目录, symlink, hard link, rename 和基础元数据操作.
- 支持分区块设备, 让 rootfs 不依赖整盘设备顺序.
- 为 `/dev` 上的块设备节点和 VFAT 测试分区共存提供基础.

## 非目标

- 不在文档中复述 ext4_rs 内部结构.
- 不承诺完整 journaling 语义.
- 不把旧的 `init_ext4_from_block_device` 描述为默认运行路径; 它只是内部/测试兼容入口.

## 模块边界

- `mod.rs`: `Ext4FileSystem::open`, superblock 预检, statfs/sync.
- `adpaters.rs`: `BlockDriver` 到 ext4_rs block interface 的适配.
- `inode.rs`: VFS inode 操作到 ext4_rs 的映射和 inode cache.
- `fs/mod.rs`: rootfs 探测和临时挂载策略.

## 关键流程

### rootfs probe

```text
list block devices
  -> prefer partition names
  -> Ext4FileSystem open
  -> mount at /
  -> vfs_lookup /bin/sh or /bin/ash
  -> accept or rollback
```

这个流程避免把 `vda1` 写死为 rootfs. 如果 QEMU 或设备注册顺序变化, 只要某个分区包含可用 shell, 仍可被选中.

### block access

```text
Ext4Inode
  -> ext4_rs
  -> BlockDeviceAdapter
  -> BlockDriver
  -> virtio block or partition device
```

分区设备由 `PartitionBlockDevice` 包装整盘设备, ext4 层看到的是从分区起点开始的逻辑块空间.

## 并发和生命周期约束

- ext4_rs 对象由内核锁保护.
- ext4 inode 和 VFS dentry 之间不能形成强引用环.
- `sync` 下推到底层块设备 flush.
- rootfs probe 的失败候选必须卸载并清理 dentry cache, 否则后续候选会看到旧根路径.

## 已知限制

- 高级 ext4 特性和崩溃恢复不是当前文档承诺范围.
- superblock 预检只用于避免明显坏镜像进入 ext4_rs.
- rootfs 判定只检查 `/bin/sh` 或 `/bin/ash`, 不验证完整用户态环境.

## 源码索引

- `os/src/fs/ext4/mod.rs`: 文件系统打开, superblock 预检, statfs/sync.
- `os/src/fs/ext4/adpaters.rs`: 块设备适配层.
- `os/src/fs/ext4/inode.rs`: ext4 inode 到 VFS inode 的映射.
- `os/src/fs/mod.rs`: `init_rootfs_from_discovered_block_devices`.
- `os/src/device/block/partition.rs`: 分区块设备包装.
