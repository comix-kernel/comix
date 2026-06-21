# Comix

Comix 是一个以 Rust 编写的教学/实验型内核项目，目前聚焦 RISC-V 64 位 QEMU virt 平台（LoongArch 目录暂为脚手架）。目标是循序搭建一套可自举、结构清晰、易于重构的内核与用户态生态，并兼容 Linux ABI 的子集。

## 当前特性

- 启动与陷入：RISC-V 裸机入口、分页开启、统一 trap 入口，用户/内核态分离；SBI 关机与 GDB 友好符号。
- 内存与任务：物理页帧分配、全局堆、SV39 地址空间、ELF 加载、用户栈构建；简单调度器与基础多任务骨架。
- 系统调用与 IPC：Linux ABI 子集（文件/管道/信号等接口在推进中），pipe/message/shared_memory/signal 框架已接入。
- 文件系统与 VFS：多层 VFS（路径解析、挂载、FD 表、文件锁）；支持 simple_fs/smfs、tmpfs、procfs、sysfs；基于 VirtIO-Block 的 Ext4 读写（ext4_rs 适配）。
- 设备与驱动：VirtIO MMIO 框架、RAMDisk、VirtIO-Block、UART console、RTC、基础网卡适配骨架，设备树读取与驱动注册。
- 用户态支持：`user/` 下的 RISC-V ELF 程序自动随内核构建并打包进根文件系统，可通过 `execve` 运行。
- 日志与调试：分级日志、早期打印、QEMU + GDB 一键调试、内核测试在 QEMU 中运行。

## 仓库布局

- document/：设计文档与开发指南（mdBook 结构），参见 [document/README.md](document/README.md)
- os/：内核 crate 与构建脚本（Makefile、build.rs、qemu-run.sh、链接脚本等）
- user/：用户态支持库与示例程序（自动被 build.rs 编译并打包）
- data/：根文件系统基础内容（busybox、init、配置等），构建 fs.img 时会被拷贝
- scripts/：工具脚本（simple_fs 打包、链接重写等）
- Makefile：顶层便捷命令（build/run/clean、Docker 构建）

## 环境依赖

- Rust nightly（已在 rust-toolchain.toml 固定）
- RISC-V 目标：`rustup target add riscv64gc-unknown-none-elf`
- QEMU：`qemu-system-riscv64`
- 构建工具：`make`、`python3`、`dd`、`truncate`、`sfdisk`、`mkfs.ext4`、`mkfs.vfat`、`rust-objcopy`（首轮构建会创建 ext4 镜像，可能较耗时/磁盘）
- 可选：Docker/DevContainer 直接复用仓库提供的镜像

## 构建与运行

```bash
# 构建内核（自动编译 user 程序并生成 rootfs 中间镜像）
make build

# 在 QEMU 运行（使用 VirtIO-Block 挂载 MBR 分区盘）
cd os && make run

# 调试：前台等待 GDB
a) cd os && make debug   # 启动 QEMU 等待 :1234
b) cd os && make gdb     # 另一个终端连接 GDB

# 在 QEMU 中运行内核测试
cd os && make test
```
提示：`os/fs-riscv.img` / `os/fs-loongarch.img` 是 build.rs 从 data/ 与 user/bin 构建出的裸 ext4 rootfs 中间产物。实际 QEMU 运行会把它组装进 MBR raw disk，并通过 VirtIO-Block 挂载该分区盘；`simple_fs.img` 当前构建为空占位，未来可切换为内存盘嵌入方案。

## OSComp 镜像布局

仓库根执行 `make all` 会生成评测用内核与磁盘镜像：

- `kernel-rv` / `kernel-la`：RISC-V 与 LoongArch ELF 内核。
- `os/fs-riscv.img` / `os/fs-loongarch.img`：裸 ext4 rootfs 中间产物，包含 `/bin`、`/sbin` 与 `/tests` 挂载点等内容。
- `disk.img` / `disk-la.img`：QEMU 实际挂载的 MBR raw disk，也是默认启动要求的磁盘形态。

`disk.img` 与 `disk-la.img` 是我们的 MBR 分区盘，分区固定为：

- 第一分区：ext4 rootfs，内容来自对应的 `os/fs-*.img`。
- 第二分区：64 MiB FAT32/VFAT 空分区，用于 OSComp `basic/mount` 与 `basic/umount` 测试。

官方测试镜像 `sdcard-rv.img` / `sdcard-la.img` 不写入 rootfs；本地运行时它们需要位于仓库根目录。`make run-rv` / `make run-la` 会将我们的 MBR 分区盘作为第一个 VirtIO 块设备 `vda`，将官方测试镜像作为第二个块设备 `vdb`。内核从 `vda1` 启动 rootfs，`rcS` 会把 `vdb` 这个裸 ext4 测试镜像挂到 `/tests` 后执行白名单 musl 测试。

内核默认会从发现到的块设备（整盘和分区）中探测 ext4 rootfs，优先尝试分区设备，并选择含 `/bin/sh` 或 `/bin/ash` 的设备挂载为 `/`，因此评测运行形态会从我们的 `vda1` 启动。`oscomp` feature 已弃用并保留为空兼容项，不再改变启动行为。

## 用户态程序

用户程序放在 user/ 下的子 crate 中，`make build`/`cargo build` 会自动：
1) 在 user/ 内执行 `make`，产物移动到 user/bin/
2) 将 user/bin/ 与 data/ 一起打包进 ext4 镜像 `/home/user/bin` 路径

若需手动构建或添加新程序，见 [user/README.md](user/README.md)。

## 文档与贡献

- 阅读文档：参见 [document/README.md](document/README.md) 和 SUMMARY 导航。
- 贡献流程：请先阅读 [CONTRIBUTING.md](CONTRIBUTING.md)，提交 PR/Issue 前确保通过 fmt/clippy/测试。

## 许可证

[GPL-3.0](LICENSE)
