# 设备与存储路径

设备层为 Comix 提供驱动注册, 设备树探测, VirtIO 传输, 块设备, 网络, 控制台, RTC 等基础能力. 对文档同步最关键的是存储路径: 块设备和分区被枚举为 `vda`, `vda1`, `vda2` 等名字, 上层 FS 再从这些候选中探测 rootfs 或挂载 VFAT 测试分区.

## 当前状态

- 所有驱动实现 `Driver`, 按类型可向下暴露 `BlockDriver`, `NetDevice`, `RtcDriver`, `SerialDriver`.
- 全局注册表包括 `DRIVERS`, `BLK_DRIVERS`, `RTC_DRIVERS`, `SERIAL_DRIVERS`.
- 设备树初始化先处理中断控制器, 再处理普通设备.
- VirtIO MMIO 是主要设备传输路径, PCI 也有部分驱动入口.
- 块设备支持整盘和 MBR/GPT 分区包装.
- sysfs 通过设备注册表构建 `/sys/class/*`, FS 初始化通过同一设备列表创建 `/dev` 节点.

## 目标

- 为 FS 层提供稳定的块设备抽象.
- 为 VFS 设备文件提供字符/块设备驱动入口.
- 让 rootfs 探测不依赖固定 virtio 设备枚举顺序.
- 让 VFAT/FAT 测试分区能作为普通分区块设备被挂载和卸载.

## 非目标

- 不在设备文档中描述具体文件系统格式.
- 不承诺完整热插拔设备管理.
- 不复制每个驱动寄存器和队列实现.

## 模块边界

- `device/mod.rs`: 驱动 trait 和全局注册表.
- `device_tree.rs`: FDT 解析, bootargs, compatible 到 probe 的分发.
- `bus/virtio_mmio.rs`: VirtIO MMIO transport 探测和设备类型分发.
- `virtio_hal.rs`: virtio-drivers 使用的 DMA/MMIO HAL.
- `block/mod.rs`: `BlockDriver` 接口.
- `block/virtio_blk.rs`: virtio block 整盘设备.
- `block/partition.rs`: MBR/GPT 分区发现和 `PartitionBlockDevice`.
- `console/`, `serial/`, `rtc/`, `net/`: 字符, 时间和网络设备来源.
- `fs/sysfs/device_registry.rs`: 设备列表投影到 sysfs 和 FS 初始化.

## 存储关键流程

### 设备发现

```text
device_tree init
  -> virtio mmio probe
  -> virtio block init
  -> BLK_DRIVERS push whole disk
  -> sysfs device_registry list_block_devices
  -> discover partitions
  -> vda, vda1, vda2 ...
```

`BLK_DRIVERS` 只保存整盘驱动. 分区设备是在 `list_block_devices` 时根据分区表动态包装出来的逻辑块设备.

### rootfs selection

```text
list block devices
  -> prefer partition names
  -> FS tries ext4
  -> accept if /bin/sh or /bin/ash exists
```

默认分区盘设计是 ext4 rootfs 和 VFAT 测试分区共存. 一般情况下 ext4 rootfs 在 `vda1`, VFAT/FAT 测试分区在 `vda2`, 但代码以内容探测为准, 不写死设备名.

### VFAT test partition

VFAT/FAT 分区通过同一 `BlockDriver` 路径进入 `os/src/fs/vfat/`. 它用于 mount/umount, statfs, flush 和跨文件系统路径解析测试, 不参与默认 rootfs 选择.

### `/dev` and `/sys`

```text
list_block_devices
  -> /sys/class/block entries
  -> /dev block nodes
```

这保证用户态能通过 `/sys/class/block/vda1` 观察分区, 也能通过 `/dev/vda1` 作为 mount source 使用.

## 并发和生命周期约束

- 驱动注册表初始化后主要读多写少.
- `PartitionBlockDevice` 持有底层整盘 `Arc<dyn BlockDriver>`, 不复制数据.
- 分区读写会检查逻辑块范围, 再偏移到底层整盘块号.
- VirtIO 设备通过内部锁串行化驱动对象访问, IRQ 路径通过 `IRQ_MANAGER` 分发.
- DMA allocation 由 `VirtIOHal` 记录物理帧范围, 释放时注意锁顺序.

## 已知限制

- 设备热插拔后 `/dev` 和 `/sys` 不是自动增量更新模型.
- 分区解析支持 primary MBR 和基础 GPT 条目, 不覆盖所有复杂分区格式.
- 分区发现假设 512 字节扇区.
- 多块设备命名使用 `vda`, `vdb` 顺序, 仍应避免在 rootfs 策略中硬编码.

## 源码索引

- `os/src/device/mod.rs`: 驱动模型和注册表.
- `os/src/device/device_tree.rs`: FDT 初始化和 probe 分发.
- `os/src/device/bus/virtio_mmio.rs`: VirtIO MMIO 设备识别.
- `os/src/device/virtio_hal.rs`: DMA/MMIO HAL.
- `os/src/device/block/mod.rs`: `BlockDriver`.
- `os/src/device/block/virtio_blk.rs`: virtio block 驱动.
- `os/src/device/block/partition.rs`: MBR/GPT 分区和分区块设备.
- `os/src/fs/sysfs/device_registry.rs`: 块设备和分区枚举.
- `os/src/fs/mod.rs`: rootfs 探测和 `/dev` 节点创建.
- `os/src/fs/vfat/`: VFAT/FAT 分区挂载路径.
- `os/src/fs/ext4/`: ext4 rootfs 挂载路径.
