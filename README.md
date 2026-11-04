# Comix

Comix 是一个以 Rust 编写的教学/实验型内核项目，支持 RISC-V 64 与 LoongArch64 的 QEMU virt 平台。目标是循序搭建一套可自举、结构清晰、易于重构的内核与用户态生态，并兼容 Linux ABI 的子集。两个架构都从公共启动流程进入 `/sbin/init`。LoongArch64 目前仍是单核路径。

## 当前特性

- 启动与陷入：RISC-V 与 LoongArch 裸机入口、分页开启、统一 trap 入口，用户/内核态分离。RISC-V 支持多核启动与核间重调度中断；LoongArch 的 IPI 仍是空实现。
- 内存与任务：按设备树中的多段 DRAM 分配物理页，区间之间的空洞不参与分配；全局堆、SV39/LoongArch 页表、ELF 加载和用户栈构建。每个 CPU 有独立调度器：实时任务（`SCHED_FIFO` / `SCHED_RR`）按优先级优先于 fair 任务，fair 任务按 `vruntime` 选择，时间片随可运行任务数变化。
- 系统调用与 IPC：Linux ABI 子集，覆盖文件、进程、内存、信号、管道和套接字。`getrusage`、`madvise`、`get_mempolicy` 提供测试所需的兼容行为，其中资源统计、内存建议和 NUMA 策略本身还没有完整实现。相对睡眠使用单调硬件时钟。
- 文件系统与 VFS：多层 VFS（路径解析、挂载、FD 表、文件锁）；支持 simple_fs/smfs、tmpfs、procfs、sysfs、VFAT；基于 VirtIO-Block 的 Ext4 读写（`third_party/ext4_rs`）。
- 设备与驱动：VirtIO MMIO/PCI、块设备与 MBR/GPT 分区、UART/console、RTC、网卡，设备树读取与驱动注册。终端 `termios` 使用 Linux asm-generic 的 36 字节布局。
- 用户态支持：`user/` 下的 ELF 程序自动随内核构建并打包进根文件系统，可通过 `execve` 运行。官方测试镜像存在时，`rcS` 会运行其中的 musl（含 LTP）和 glibc 脚本。
- 日志与调试：分级日志、早期打印、QEMU + GDB 调试、内核测试在 QEMU 中运行。

## 仓库布局

- document/：设计文档与开发指南（mdBook 结构），参见 [document/README.md](document/README.md)
- os/：内核 crate 与构建脚本（Makefile、build.rs、qemu-run.sh、链接脚本等）
- user/：用户态支持库与示例程序（自动被 build.rs 编译并打包）
- data/：根文件系统基础内容（busybox、init、配置等），构建 fs.img 时会被拷贝
- scripts/：工具脚本（simple_fs 打包、链接重写等）
- Makefile：顶层便捷命令（build/run/clean、Docker 构建）

## 环境依赖

- Rust nightly（已在 rust-toolchain.toml 固定）
- 目标：`rustup target add riscv64gc-unknown-none-elf loongarch64-unknown-none`
- QEMU：`qemu-system-riscv64`、`qemu-system-loongarch64`
- 构建工具：`make`、`python3`、`dd`、`truncate`、`sfdisk`、`mkfs.ext4`、`mkfs.vfat`、`rust-objcopy`（首轮构建会创建 ext4 镜像，可能较耗时/磁盘）
- 可选：Docker/DevContainer 直接复用仓库提供的镜像

## 构建与运行

```bash
# 构建当前架构的内核。build.rs 只生成 rootfs 中间镜像，不编译 user/
make build

# 评测形态：生成 kernel-rv、kernel-la、disk.img、disk-la.img
make all

# 挂上官方测试镜像并启动。镜像默认找 ../CCYOS/sdcard-*.img
make run-rv
make run-la

# 在 os/ 里用调试脚本启动。LoongArch 需要 ARCH=loongarch
cd os && make run
cd os && make run ARCH=loongarch

# 调试：前台等待 GDB
a) cd os && make debug   # 启动 QEMU 等待 :1234
b) cd os && make gdb     # 另一个终端连接 GDB

# 在 QEMU 中运行内核测试
cd os && make test
```

`os/fs-riscv.img` / `os/fs-loongarch.img` 是 build.rs 分别从 `data/risc-v_musl` 和 `data/loongarch_musl` 生成的裸 ext4 中间产物，并预先放好 `/tests`、`/lib` 等目录。实际 QEMU 运行会把它组装进 MBR raw disk，再通过 VirtIO-Block 挂载。`build.rs` 嵌入内核的 `simple_fs.img` 是空占位，不包含 `user/bin`。

## OSComp 镜像布局

仓库根执行 `make all` 会生成评测用内核与磁盘镜像：

- `kernel-rv` / `kernel-la`：RISC-V 与 LoongArch ELF 内核。
- `os/fs-riscv.img` / `os/fs-loongarch.img`：裸 ext4 rootfs 中间产物，包含 `/bin`、`/sbin` 与 `/tests` 挂载点等内容。
- `disk.img` / `disk-la.img`：QEMU 实际挂载的 MBR raw disk，也是默认启动要求的磁盘形态。

`disk.img` 与 `disk-la.img` 是我们的 MBR 分区盘，分区固定为：

- 第一分区：ext4 rootfs，内容来自对应的 `os/fs-*.img`。
- 第二分区：64 MiB FAT32/VFAT 空分区，用于 OSComp `basic/mount` 与 `basic/umount` 测试。

官方测试镜像 `sdcard-rv.img` / `sdcard-la.img` 不写入 rootfs，也不需要复制到本仓库。`make run-rv` / `make run-la` 默认优先使用相邻 `../CCYOS/` 目录下的测试镜像，也可通过 `TESTIMG_RV=/path/to/sdcard-rv.img` 或 `TESTIMG_LA=/path/to/sdcard-la.img` 指定任意本地路径。镜像根目录包含 `musl/` 与 `glibc/`。

两套架构的块设备号不一样：

- RISC-V：`os/qemu-run.sh` 把分区盘放在 virtio-mmio bus.1，使它注册为 `/dev/vda`；官方测试镜像放在 bus.0，注册为 `/dev/vdb`。`data/risc-v_musl/etc/init.d/rcS` 把 `/dev/vdb` 挂到 `/tests`。
- LoongArch：仓库根 `make run-la` 先挂官方测试镜像，再挂分区盘，使测试镜像成为 `/dev/vda`、分区盘成为 `/dev/vdb`。`data/loongarch_musl/etc/init.d/rcS` 只探测 `/dev/vda`。`os/qemu-loongarch-run.sh`（`cd os && make run ARCH=loongarch`）把两块盘的顺序反了过来，因此不会走到同一条 `rcS` 探测路径。

内核从发现到的块设备（整盘和分区）中探测 ext4 rootfs，优先尝试分区设备，并选择含 `/bin/sh` 或 `/bin/ash` 的设备挂载为 `/`。rootfs 因此不一定是 `vda1`。`oscomp` feature 已弃用并保留为空兼容项，不再改变启动行为。

`rcS` 找到官方镜像后，先运行 `/tests/musl` 下的脚本，并把 `ltp_testcode.sh` 放在其他 musl 脚本之后；再运行 `/tests/glibc` 下的脚本。`iozone_testcode.sh` 和 `unixbench_testcode.sh` 会跳过。glibc 运行前会把动态链接器及常用库链接到 `/lib`。只要有一组测试实际跑过，脚本结束时会关机。

## 用户态程序

`user/` 是一组可选的 RISC-V `no_std` 示例，不参与 `make build` 或评测 rootfs。需要时手动执行 `make build-user`，产物在 `user/bin/`。`make pack-simple-fs` 会把这些产物打进 `os/simple_fs.img`，这是调试用 SimpleFS 镜像，不是 ext4 rootfs。

评测 rootfs 的用户态来自 `data/risc-v_musl` 与 `data/loongarch_musl` 里的 busybox，以及运行时挂上的官方测试镜像。

若需添加示例程序，见 [user/README.md](user/README.md)。

## 文档与贡献

- 阅读文档：参见 [document/README.md](document/README.md) 和 SUMMARY 导航。
- 贡献流程：请先阅读 [CONTRIBUTING.md](CONTRIBUTING.md)，提交 PR/Issue 前确保通过 fmt/clippy/测试。

## 许可证

[GPL-3.0](LICENSE)
