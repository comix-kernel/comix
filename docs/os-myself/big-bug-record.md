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

## 2026-06-23 iperf TCP 吞吐从几 KB/s 提升到数百 Mbit/s

### 现象

最初 iperf 组能启动，但 TCP 分数几乎没有贡献：

- `BASIC_TCP` 只发送 4KiB，sender 约 16Kbit/s，receiver 为 0。
- `REVERSE_TCP` sender 也只有 4KiB，receiver 为 0。
- `PARALLEL_TCP` 第二条以后连接报 `Connection refused`。
- UDP 能跑通，说明基本 socket、loopback 地址和 iperf 控制连接不是完全坏掉；问题集中在 TCP 数据面和监听队列语义。

### 关键发现

第一层瓶颈是监听语义。iperf3 `-P 5` 会为同一个 server port 建多条 TCP data stream。smoltcp 的 TCP socket 没有 Linux 那种单 socket backlog 队列模型，监听端需要准备多个 TCP socket 才能同时接住多个 SYN。原实现只有一个 listener handle，所以第一条连接建立后，后续 stream 容易被拒绝。

第二层瓶颈是 loopback MTU 和 adapter RX buffer 不一致。loopback 设备暴露 `mtu=65535`，smoltcp 因此会发接近 64KiB 的 TCP/IP 包；但 `NetDeviceAdapter` 原来只有 2048 字节接收缓冲。结果大 TCP frame 在 adapter 层直接被丢掉，表现为客户端以为发出了少量数据，服务端几乎收不到。

第三层是 syscall 和轮询节奏。iperf3 会传大 buffer；如果每次 syscall 都按用户长度一次性分配/copy，会增加内核堆压力，也让 loopback 队列 drain 不及时。写入后不主动 poll，会把推进 TCP 状态机的工作推迟到后续调度点，吞吐很差。

### 处理步骤

这轮 TCP 相关修复按小步提交推进：

- `da0d56c tests: stage busybox for iperf script`：iperf 脚本里会调用 BusyBox 工具，staging 只复制 `iperf3` 不够，先保证测试脚本自身能稳定运行。
- `56c01e8 net: enlarge socket buffers for iperf`：TCP/UDP buffer 扩到 256KiB，避免 iperf 数据面频繁因为小 buffer 进入 WouldBlock。
- `de092e8 net: chunk socket syscall buffers`：`send/recv` 单次内核复制限制到 64KiB，控制临时分配和 copy 成本。
- `e63c8a5 net: drain loopback after socket writes`：socket write/sendto 成功后 bounded poll，尽快把 loopback Tx frame 回灌到 Rx 并推进 smoltcp 状态机。
- `eb9d8ce net: support parallel tcp listeners`：为监听 socket 维护一组 spare TCP listener，accept 时把已建立连接交给新 fd，并补充新的 listener，解决 `PARALLEL_TCP` 连接拒绝。
- `491634c net: reduce iperf socket log noise`：把热路径 socket/TCP 连接日志从 info 降到 debug，避免串口日志本身拖慢性能。
- `aae6897 net: size adapter rx buffer for mtu`：adapter RX buffer 改为按 `device.mtu() + Ethernet header` 分配，修复 64KiB loopback TCP frame 被 2048B buffer 丢弃的问题。
- `474b6ea net: cap tcp buffers below unstable window`：验证 UDP 改动时复现 smoltcp `SeqNumber` subtraction underflow，最终把 TCP buffer 从 256KiB 收到 128KiB-1，避开不稳定的大窗口组合。

### 决策和取舍

没有直接 patch smoltcp。这里的问题主要是我们给 smoltcp 的设备能力、buffer 和 listen 模型不一致，先修本内核适配层更稳，也更容易解释。

TCP buffer 起初选 256KiB，是为了减少 WouldBlock 并提高窗口；这一版峰值很好，`BASIC_TCP`/`REVERSE_TCP` 能到约 490Mbit/s，`PARALLEL_TCP` 能到约 709Mbit/s。但后续在完整 iperf-only 复测中，`PARALLEL_TCP` 五条连接建立后触发 smoltcp sequence number underflow panic。64KiB 以内能稳定，但单流 TCP 只剩约 70Mbit/s。最终选 `128KiB-1`：仍然明显高于原始 4KiB/0 receiver，且完整 iperf 组稳定通过。

listener pool 做了上限，不直接相信 iperf 传入的巨大 backlog。当前逻辑把用户 backlog clamp 到 128，实际 spare listener pool clamp 到 16，足够覆盖 iperf `-P 5`，同时避免一次 listen 分配过多 TCP buffer。

保留 loopback 的大 MTU，而不是降回 1500。降 MTU 能减少单帧内存，但 TCP throughput 会被更多包处理开销限制。真正的问题是 adapter 宣告大 MTU 却没有同等大小的 RX buffer，因此修 buffer 更符合设备模型。

### 验证

临时把 RISC-V `rcS` 改成只跑 `iperf_testcode.sh`，构建 release 内核，用临时分区盘 `/tmp/ccyos-disk-udpq-tcp128k.img` 跑官方 RISC-V musl iperf 组。

最终采用 128KiB-1 TCP buffer 后，最新一次验证中 TCP 三项都通过：

- `BASIC_TCP`：sender 71.7MiB / 300Mbit/s，receiver 71.4MiB / 299Mbit/s。
- `PARALLEL_TCP`：5 stream 全部连接成功，SUM sender 102MiB / 424Mbit/s，receiver 102MiB / 418Mbit/s。
- `REVERSE_TCP`：sender 72.2MiB / 302Mbit/s，receiver 72.0MiB / 302Mbit/s。

这说明最初 “4KiB / receiver 0” 和 “parallel connection refused” 两个 TCP 主问题已经解决。TCP 的峰值为了稳定性放弃了 256KiB buffer 下的最高数字，但所有 TCP 子项稳定通过，且吞吐仍是原始结果的数量级提升。

## 2026-06-23 iperf UDP 单流和反向模式高丢包

### 现象

TCP 主问题解决后，UDP 仍然拖分：

- `BASIC_UDP` receiver 约 46Mbit/s，丢包约 58%-61%。
- `REVERSE_UDP` receiver 约 48Mbit/s，同样有大量丢包。
- `PARALLEL_UDP` 反而能到约 137Mbit/s 且 0% loss。

这个现象说明链路、loopback MTU 和基本 UDP socket 不是完全坏掉；单 fd 收包路径比多 stream 分摊路径更容易溢出。

### 关键发现

本内核对 UDP 做了 per-port smoltcp socket 加 per-fd 队列的分发。smoltcp 共享 socket 收到包以后，`udp_dispatch_drain_locked()` 会复制到目标 `SocketFile` 的 `udp_rx_queue`，用户态再从这个队列 `recvfrom()`。

原来的 `UDP_RXQ_CAP = 64` 太小。iperf UDP 使用极高目标带宽发包，发送端在短时间内产生的 datagram 比接收线程 drain 得更快；队列满后 `udp_push()` 直接返回 false，后续 datagram 被丢掉。`PARALLEL_UDP` 因为 5 条 stream 分散到多个 fd 队列，每个队列压力小，所以表现比单流好。

试过把 UDP 队列固定预分配到 512。这个版本把 `BASIC_UDP` 提到 77.8Mbit/s 且 0% loss，但 `PARALLEL_UDP` 随后触发 `memory allocation of 1064960 bytes failed`。原因是每个 UDP 队列项包含最大 2048 字节 payload 缓冲，512 项接近 1MiB；所有 UDP socket 一创建就预分配，会把 parallel 场景的内存压力放大。

### 处理步骤

- `f4905d9 net: grow udp receive queues lazily`：UDP per-fd 队列初始仍为 64，只有队列满时才尝试翻倍扩容，最大 512。
- 如果扩容失败或已达上限，队列丢弃最旧 datagram 再放入新 datagram。这样在内存紧张时仍然有 UDP 丢包，但不会因为固定大队列把内核堆打爆，也更偏向保留较新的 iperf 数据。

### 决策和取舍

没有把所有 UDP socket 都预分配 512 项。固定大队列能改善单流 loss，但并行时内存占用不可控；lazy growth 只让真正承压的 socket 付出内存成本。

没有把 UDP 队列做成无限增长。iperf `-u -b 1000G` 本质上会制造远超内核处理能力的 burst；无限增长只是把丢包变成内存耗尽。512 是这次本地验证中能消掉单流丢包、又不会让 parallel UDP 分配失败的上限。

丢弃策略从“队列满就丢新包”变成“扩不动时丢旧包”。这对 UDP 是可接受取舍：UDP 本来不保证可靠性，保留较新的 datagram 对 iperf 的实时统计和反向模式更有价值。

### 验证

临时把 RISC-V `rcS` 改成只跑 `iperf_testcode.sh`，构建 release 内核并用临时分区盘 `/tmp/ccyos-disk-udpq-tcp128k.img` 跑官方 RISC-V musl iperf 组。最新一次完整 iperf-only 验证全部通过：

- `BASIC_UDP`：sender 25.9MiB / 109Mbit/s，receiver 25.9MiB / 108Mbit/s，0% loss。
- `PARALLEL_UDP`：SUM sender 36.6MiB / 154Mbit/s，receiver 36.5MiB / 153Mbit/s，0% loss。
- `REVERSE_UDP`：sender 19.9MiB / 82.7Mbit/s，receiver 19.8MiB / 82.8Mbit/s，0% loss。

这说明 UDP 的主要问题是 per-fd 接收队列容量和内存策略，而不是 UDP 校验、地址绑定或 loopback 设备本身。
