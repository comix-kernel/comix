# SimpleFS

SimpleFS 是编译期嵌入的只读测试文件系统. 当前它作为 rootfs 探测失败时的 fallback, 也用于不依赖外部磁盘镜像的测试路径.

## 当前状态

- 源码位于 `os/src/fs/simple_fs.rs`.
- 镜像由构建阶段生成并通过 `include_bytes!` 嵌入内核.
- 启动时镜像被放入 `RamDisk`, 再解析成 `SimpleFsInode` 树.
- 支持多级路径, 普通文件和目录.
- 运行时只读.

## 目标

- 在没有可用 ext4 rootfs 时保持系统可启动或可测试.
- 提供稳定, 小型, 可嵌入的测试文件树.
- 避免早期启动完全依赖 virtio block 或分区盘.

## 非目标

- 不作为正式持久化 rootfs 格式.
- 不支持运行时写入.
- 不复刻 ext4 或 FAT 的磁盘结构.

## 模块边界

- `simple_fs.rs`: 镜像解析, inode 树, `FileSystem` 实现.
- `fs/mod.rs`: `init_simple_fs` fallback 入口.
- `device/block/ram_disk.rs`: 嵌入镜像的块设备承载.
- build 脚本: 生成 `SIMPLE_FS_IMAGE`.

## 关键流程

```text
include bytes image
  -> RamDisk
  -> SimpleFs from_ramdisk
  -> mount at /
```

默认优先尝试分区盘 ext4 rootfs. SimpleFS 只在 rootfs 探测失败时作为回退路径.

## 并发和生命周期约束

- 文件内容驻留内存, 无同步到磁盘路径.
- 只读语义应在 inode 操作中保持一致.
- 镜像大小和内容由构建产物决定, 不是运行时配置.

## 已知限制

- 只读.
- 文件系统格式只服务内核测试和 fallback.
- 元数据语义较简单.

## 源码索引

- `os/src/fs/simple_fs.rs`: SimpleFS 实现.
- `os/src/fs/mod.rs`: `init_simple_fs`.
- `os/src/device/block/ram_disk.rs`: RamDisk.
