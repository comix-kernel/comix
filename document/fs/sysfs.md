# SysFS

SysFS 把设备注册表和内核属性暴露为 `/sys`. 它是冷插拔构建的伪文件系统, 当前主要用于设备发现和调试.

## 当前状态

- 源码位于 `os/src/fs/sysfs/`.
- `SysFS::init_tree` 创建 `/sys/class`, `/sys/devices`, `/sys/kernel` 等目录.
- `/sys/class/block` 根据块设备和分区列表创建符号链接.
- `/sys/block` 是指向 `class/block` 的兼容 symlink.
- `device_registry.rs` 复用设备层全局注册表, 不创建另一套设备来源.

## 目标

- 为用户态和内核调试提供统一设备视图.
- 让块设备, 网络设备, tty, input, rtc 等类别能被枚举.
- 为 FS rootfs 探测和 `/dev` 创建提供设备列表来源.

## 非目标

- 不实现完整 Linux sysfs 属性写入模型.
- 不承诺设备热插拔后自动增量更新 sysfs 树.
- 不在文档中复制所有 builder 生成的节点.

## 模块边界

- `sysfs.rs`: 文件系统实例, 根目录结构, builder 调度.
- `inode.rs`: 目录, 属性文件和 symlink inode.
- `device_registry.rs`: 从 `DRIVERS`, `BLK_DRIVERS`, `RTC_DRIVERS` 等全局表生成设备信息.
- `builders/`: 各类 `/sys` 子树构建器.

## 关键流程

### cold build

```text
SysFS new
  -> create base directories
  -> build platform devices
  -> build class symlinks
  -> mount at /sys
```

设备应先在 device 层完成注册, sysfs 初始化再读取注册表构建树.

### block devices

`list_block_devices` 会为每个 virtio block 设备生成 `vda`, `vdb` 等整盘名称, 并解析 MBR/GPT 分区生成 `vda1`, `vda2` 等逻辑分区设备. 这些名字会同时影响 `/sys/class/block` 和 `/dev` 节点创建.

## 并发和生命周期约束

- 当前 sysfs 树按初始化时设备注册表冷构建.
- 属性文件读取应从设备或内核状态生成当前值.
- 分区设备是包装整盘块设备的 `Arc<dyn BlockDriver>`, 生命周期依赖底层驱动全局注册表.

## 已知限制

- 写属性和热插拔更新能力有限.
- sysfs 结构只覆盖当前内核已有设备类别.
- 分区解析依赖块大小和分区表可读性.

## 源码索引

- `os/src/fs/sysfs/sysfs.rs`: `SysFS` 初始化和树构建流程.
- `os/src/fs/sysfs/inode.rs`: sysfs inode 类型.
- `os/src/fs/sysfs/device_registry.rs`: 设备列表和分区枚举.
- `os/src/fs/sysfs/builders/`: class/devices/kernel 子树构建器.
- `os/src/device/block/partition.rs`: MBR/GPT 分区解析.
