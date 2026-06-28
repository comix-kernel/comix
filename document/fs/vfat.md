# VFAT/FAT

VFAT/FAT 支持位于 `os/src/fs/vfat/`. 这是当前 FAT 相关实现的正式入口, 用于 VFAT/FAT mount 兼容路径和分区 mount/umount 测试.

## 当前状态

- `VfatFileSystem` 实现 VFS `FileSystem`.
- `VfatInode` 把 FAT/VFAT 目录和文件操作映射到 VFS `Inode`.
- `FatBlockDevice` 把内核 `BlockDriver` 的整块读写适配为 `fatfs` 需要的字节流读写.
- `fs_type` 返回 `vfat`, 但底层通过 `fatfs` 支持 FAT 家族卷.
- 默认分区盘中 VFAT 分区用于 mount/umount 测试, 不参与 rootfs 自动选择.

## 目标

- 让 FAT/VFAT 分区可以挂载到 VFS 路径树.
- 支持常见文件和目录读写, 便于与主机制作的 FAT 分区交换测试数据.
- 验证块设备分区, mount stack, umount, statfs 和设备 flush 路径.
- 保持和 Linux 用户习惯一致的 `vfat` 类型名.

## 非目标

- 不作为默认 rootfs.
- 不在文档中描述 FAT 表布局和长文件名编码细节.
- 不承诺完整 Linux vfat mount option 集合.

## 模块边界

- `mod.rs`: VFAT 模块入口和公开类型.
- `fs.rs`: 挂载实例, fatfs 打开, statfs, sync, umount.
- `inode.rs`: 文件, 目录, lookup, create, readdir 等 inode 适配.
- `adapter.rs`: 任意字节范围 I/O 到 block I/O 的转换.
- `device/block/partition.rs`: 为 VFAT 分区提供逻辑块设备.

## 关键流程

### mount

```text
/dev/vda2 or another FAT partition
  -> BlockDriver or PartitionBlockDevice
  -> FatBlockDevice
  -> fatfs FileSystem
  -> VfatFileSystem
  -> MOUNT_TABLE
```

初始化代码默认不会把 VFAT 选为 `/`. 测试或用户路径可以把它挂载到 `/mnt` 等目录, 用于覆盖 mount/umount 和跨文件系统路径解析.

### unaligned I/O

`fatfs` 以字节流方式读写, 但内核块设备只接受块 I/O. `FatBlockDevice` 对非对齐写入执行 read-modify-write, 以保留同一块中未覆盖的数据.

### synchronization

`VfatFileSystem::sync` 会重新打开 fatfs 视图完成同步检查, 然后 flush 底层设备. `umount` 当前走 sync 路径, 确保 FAT 侧状态和块设备状态尽量落盘.

## 并发和生命周期约束

- `VfatState` 使用锁串行化对 fatfs 的访问.
- 每次操作会打开一个 fatfs 视图, 执行闭包, 再 unmount 该视图.
- 块设备边界由 `FatBlockDevice` 检查, 越界转换为 VFS 错误.
- VFAT inode 不能依赖 Unix inode 号和权限模型的完整语义.

## 已知限制

- Linux vfat 的 mount options, codepage 和大小写策略尚未完整暴露.
- FAT 没有 Unix inode/权限模型, statfs 和 metadata 会有适配值.
- 设计目标是兼容 FAT/VFAT 卷读写和 mount 测试, 不是替代 ext4 rootfs.

## 源码索引

- `os/src/fs/vfat/mod.rs`: 模块入口.
- `os/src/fs/vfat/fs.rs`: `VfatFileSystem` 和 fatfs 生命周期.
- `os/src/fs/vfat/inode.rs`: `VfatInode`.
- `os/src/fs/vfat/adapter.rs`: `FatBlockDevice`.
- `os/src/fs/tests/vfat/mod.rs`: VFAT 适配和 VFS 行为测试.
- `os/src/device/block/partition.rs`: 分区块设备.
