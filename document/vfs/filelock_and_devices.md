# 文件锁与设备节点

本页覆盖两个 VFS 边界能力: advisory 文件锁和设备文件. 二者都挂在 VFS 命名空间中, 但真实状态分别由全局锁表和设备驱动层维护.

## 当前状态

- `file_lock.rs` 提供全局文件锁管理器, 支持按文件范围记录共享/排他锁.
- 锁语义是 advisory, 只有遵守锁协议的调用路径才会受影响.
- `dev.rs` 提供 major/minor 设备号工具.
- `devno.rs` 保存字符/块设备主设备号约定和驱动注册.
- 字符设备和块设备通过专门的 `File` 实现进入驱动.
- `/dev` 节点由 FS 初始化阶段在根文件系统上创建.

## 目标

- 让文件锁和设备访问通过普通 fd 模型暴露.
- 让 `/dev/null`, `/dev/zero`, `/dev/tty`, `/dev/vda`, `/dev/vda1` 等节点可通过路径打开.
- 让块设备可直接作为文件访问, 也可作为 ext4/VFAT 的底层存储.
- 让锁生命周期能跟随进程退出或 fd 清理释放.

## 非目标

- 不提供强制锁.
- 不实现完整 devtmpfs.
- 不在本页记录全部 Linux 设备号.
- 不把具体驱动寄存器操作写入 VFS 文档.

## 模块边界

- `file_lock.rs`: 锁表, 冲突检测, 释放.
- `dev.rs`: 设备号编码/解码.
- `devno.rs`: 设备主号约定和驱动注册表.
- `impls/char_dev_file.rs`: 字符设备 file 适配.
- `impls/blk_dev_file.rs`: 块设备 file 适配.
- `os/src/fs/mod.rs`: `/dev` 目录和设备节点创建.
- `os/src/device/`: 真实驱动和设备注册表.

## 关键流程

### advisory lock

```text
fcntl lock request
  -> File identity
  -> global lock table
  -> conflict check
  -> record or report conflict
```

锁不直接改变 inode 读写路径. 它只为系统调用提供协作式同步状态.

### mknod and open device

```text
/dev/vda1 dentry
  -> inode type BlockDevice
  -> rdev major minor
  -> BlockDeviceFile
  -> BlockDriver
```

字符设备同理, 但进入字符驱动表. VFS 不关心底层是串口, 控制台还是内存伪设备, 只通过设备号选择驱动.

### rootfs 后创建 `/dev`

当前 rootfs 探测成功后, FS 初始化会确保 `/dev` 存在, 再创建固定字符设备节点和从 `list_block_devices` 枚举得到的块设备节点. 分区设备如 `vda1`, `vda2` 会和整盘 `vda` 一起出现.

## 并发和生命周期约束

- 文件锁表是全局状态, 需要按 owner 释放, 防止进程退出后残留.
- 设备 file 持有 dentry/inode, 真实驱动由全局设备注册表持有.
- 块设备读写必须遵守底层 `BlockDriver` 的 block size 和边界.
- 设备节点 inode 的 `rdev` 是 VFS 到驱动层的稳定桥接字段.

## 已知限制

- 文件锁不强制拦截所有读写.
- 设备节点创建是初始化时的静态扫描, 不是真正热插拔 devfs.
- 字符设备集合较小, 主要覆盖内核当前需要的 null/zero/random/tty/console/rtc.
- 分区设备依赖 MBR/GPT 解析和块设备 512 字节扇区假设.

## 源码索引

- `os/src/vfs/file_lock.rs`: advisory lock 管理.
- `os/src/vfs/dev.rs`: 设备号工具.
- `os/src/vfs/devno.rs`: 主设备号和驱动注册.
- `os/src/vfs/impls/char_dev_file.rs`: 字符设备 file.
- `os/src/vfs/impls/blk_dev_file.rs`: 块设备 file.
- `os/src/fs/mod.rs`: `/dev` 节点创建.
- `os/src/device/block/mod.rs`: `BlockDriver`.
- `os/src/device/block/partition.rs`: 分区块设备.
- `os/src/device/console/`, `os/src/device/serial/`, `os/src/device/rtc/`: 字符类设备来源.
