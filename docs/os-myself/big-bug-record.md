# 大问题记录

## 2026-06-23 评测打包把 BusyBox symlink 当成真实文件重复计数

### 现象

项目仓库里原来直接跟踪了 `data/risc-v_musl/bin` 和 `data/loongarch_musl/bin` 下的大量 BusyBox applet 符号链接。正常的 `git archive` 或普通 zip 不会把这些链接当成完整文件复制，但部分评测侧打包/扫描逻辑可能会跟随 symlink，把每个链接都当成一份 `busybox` 二进制内容重新计数。

同时，构建脚本原来会生成 4GiB 级别的 rootfs/ext4 镜像。评测机拉取仓库后执行 `make all`，如果它的中间打包或磁盘统计对 symlink/大镜像处理不佳，就会出现压缩体积和磁盘写入量远超实际需要的问题。

### 原因

BusyBox applet 的链接森林只是在运行时需要，不应该作为 Git 仓库里的真实追踪对象大量存在。评测环境实际只要求仓库提供源码和构建逻辑，运行时 rootfs 可以由 `make all` 临时生成。

`disk.img` 是 QEMU 运行时挂载给系统用的辅助磁盘；`kernel-rv`/`kernel-la` 是裸核 ELF。它们不是同一种东西。评测机会执行 `make all`，然后拿 `kernel-rv`/`kernel-la` 启动 QEMU，必要时再挂载我们生成的 `disk.img`/`disk-la.img`。

### 处理

已经把 BusyBox applet symlink 从 Git 追踪内容里移除，改成保存 `symlinks.manifest`。构建时 `build.rs` 会把 `data/{risc-v_musl,loongarch_musl}` 复制到临时 rootfs，再按 manifest 重建 symlink，保证运行时 rootfs 仍然有完整 BusyBox applet。

同时把 rootfs 镜像大小从 4096MiB 降到 256MiB。当前 `make all` 仍然会生成带分区表的 `disk.img` 和 `disk-la.img`，其中 Linux rootfs 分区是 256MiB，VFAT 分区是 64MiB，整体约 322MiB。这个大小足够当前 rootfs 使用，也显著降低评测机写盘和打包压力。

### 验证

使用本地 Docker 镜像 `zhouzhouyi/os-contest:20260510` 执行过 `make all`，确认 RISC-V/LoongArch 内核和磁盘镜像都能生成。对应变更已在提交 `18c8795 Reduce rootfs image size and rebuild busybox links` 中记录。

## 2026-06-23 官方测试盘 ext4 读写太慢，basic-musl 卡在前半段

### 现象

`make run-rv` 挂载官方测试盘 `/dev/vdb` 到 `/tests` 后，系统能进入 `basic-musl`，但执行非常慢。早期日志经常停在：

- `Testing chdir`
- `Testing getpid`
- `Testing mkdir_`
- `Testing mount`
- `Testing unlink`

这不是单纯的 syscall 不支持问题。许多子项已经能打印成功结果，但从官方 ext4 测试盘加载 ELF、动态链接器、脚本、目录项，以及在测试目录里写入/删除文件，会消耗大量 QEMU 时间。

### 原因

主要有三层：

1. ext4 层以 4096B block 为单位读数据，但底层 VirtIO block sector 是 512B。原来的适配路径会把一次 4KiB 读取拆成多次 512B 请求，动态加载器和重复 exec 会放大这个成本。
2. 官方测试盘是 4GiB raw ext4 镜像，测试目录在 `/tests/musl`。直接在这个盘上运行写入型测试时，目录创建、unlink、mount/umount 测试路径都会落到慢速 ext4 设备上。
3. 一次性把 basic/busybox/lua/iperf 全部复制到 tmpfs 虽然能减少后续读写，但预复制本身会吃掉大量评测时间窗口。

### 处理

这次做了几类优化：

- 给 `BlockDriver` 增加连续块批量读写接口，默认实现仍然循环单块读写，VirtIO MMIO/PCI 驱动覆盖为真正的 `read_blocks`/`write_blocks`。
- 分区块设备把批量读写转发到底层设备，并自动加上分区起始 offset。
- ext4 adapter 增加小型 4KiB 读缓存。对 aligned ext4 block read，直接用一次连续 sector 读替代 8 次 512B 单扇区读；写入时会让重叠缓存失效。
- `mount -t tmpfs` 支持普通目录挂载，后续可以更灵活地把测试工作目录放进 tmpfs。
- `rcS` 改成按测试组懒 staging：运行 `basic_testcode.sh` 前只复制 `basic`、`basic_testcode.sh`、`busybox` 到 `/tmp/musl`；如果 basic 能跑完，再继续复制 busybox/lua/iperf 对应依赖。这样不会在 basic 之前先复制所有组。
- 给 tmpfs 补了 `chmod`/`chown` 元数据更新，避免 BusyBox `cp -R` 保留权限时报 `Not supported`，也让复制到 tmpfs 后的文件权限更接近原测试盘。

### 验证

所有验证都用 Docker 镜像 `zhouzhouyi/os-contest:20260510`，没有使用本机环境直接验证。

- `cargo fmt --manifest-path os/Cargo.toml --check` 通过。
- `make all` 通过，重新生成 `kernel-rv`、`kernel-la`、`disk.img`、`disk-la.img`。
- `timeout 240s make run-rv` 能挂载 `/dev/vdb`，把 basic 组 staging 到 `/tmp/musl`，进入 `#### OS COMP TEST GROUP START basic-musl ####`，且不再出现 `./busybox: not found`。
- 最终 240 秒窗口内跑到 `Testing umount` 开始处。之前一次性 staging 全部白名单只能到 `getpid` 附近；直接从官方 ext4 测试盘运行在写入/删除类测试处明显更慢。

### 残余问题

当前优化还不是最终形态。basic 仍然无法在 240 秒本地窗口内完整跑完，最后停在 `umount` 附近；后续如果继续提速，优先看：

1. ELF/动态链接器文件页缓存，而不是只缓存 ext4 block。
2. 减少 `mount`/`umount` 测试里的 VFAT 初始化成本。
3. 对目录项查找和路径解析加缓存，减少 exec 高频路径的重复 ext4 访问。
4. 如果评测机总时间更长，可以保留当前按组懒 staging；如果只追 basic 分数，可以进一步只运行 basic 组，避免后续组影响关机和输出。
