# testsuits-for-oskernel 支持记录

记录时间：2026-06-22

本记录结合两类信息：

- 静态检查：脚本、测试源码、ELF 元数据和内核 syscall/文件系统/进程/网络实现。
- 运行验证：在 `zhouzhouyi/os-contest:20260510` 中执行 `make all PROFILE=release`，并按官方 RISC-V QEMU 形态挂载 `sdcard-rv.img` 为第一个 virtio-blk、`disk.img` 为第二个 virtio-blk。

本次运行验证使用 `timeout --foreground 300s`，因此只能说明 300 秒窗口内的进度，不能等同于完整 suite 通过。

## 自动测试入口

当前自动入口在：

- `data/risc-v_musl/etc/init.d/rcS`
- `data/loongarch_musl/etc/init.d/rcS`

`rcS` 会先挂载 `/proc`、`/sys`、`/tmp`、`/dev`，然后挂载官方测试镜像到 `/tests`，只收集 `/tests/musl` 顶层白名单脚本执行。不会递归进入子目录，也不会自动执行 `/tests/glibc`。

官方形态下，测试内容来自第一块 virtio-blk 测试盘，不来自我们生成的 `disk.img`。`disk.img` 作为第二块可选磁盘挂载，当前主要为 rootfs 和 `/dev/vda2` VFAT 辅助分区提供支持。

当前自动入口白名单为：

- `basic_testcode.sh`
- `busybox_testcode.sh`
- `iperf_testcode.sh`
- `lua_testcode.sh`

## basic-musl 结论

`basic_testcode.sh` 进入 `./basic` 后执行 `run-all.sh`。`basic/run-all.sh` 会顺序运行以下子项：

```text
brk chdir clone close dup2 dup execve exit fork fstat getcwd getdents
getpid getppid gettimeofday mkdir_ mmap mount munmap openat open pipe
read sleep times umount uname unlink wait waitpid write yield
```

本次官方 RISC-V 形态 300 秒运行结果：

- `make all PROFILE=release` 通过，生成 `kernel-rv`、`kernel-la`、`disk.img`、`disk-la.img`。
- QEMU 能启动、挂载 rootfs、挂载官方测试盘 `/dev/vdb` 到 `/tests`，并进入 `/tests/musl/basic_testcode.sh`。
- 300 秒窗口内跑到 `unlink`：

```text
Testing uname :
========== START test_uname ==========
Uname: Linux comixhost 5.10.0 #1 SMP Mon Jan 1 00:00:00 UTC 2025 riscv64 localdomain
========== END test_uname ==========
Testing unlink :
========== START test_unlink ==========
qemu-system-riscv64: terminating on signal 15 from pid 7 (timeout)
```

已实测输出 success/正常结束的 basic 子项：

```text
brk chdir clone close dup2 dup execve exit fork fstat getcwd getdents
getpid getppid gettimeofday mkdir_ mmap mount munmap openat open pipe
read sleep times umount uname
```

300 秒窗口内未完成确认的 basic 子项：

```text
unlink wait waitpid write yield
```

其中 `unlink` 已进入测试但未在 300 秒内继续输出。结合当前 ext4 路径，主要风险是官方测试盘上直接执行相对写入/删除，触发 ext4 目录项、inode bitmap、block group、superblock 的真实写入；`ext4_rs` 自身还存在写前整块读取和写后读回校验，导致 I/O 放大。本次已优化 adapter 层的整扇区写入，但实测仍不足以让 `unlink` 在 300 秒窗口内完成。

`mount/umount` 需要运行镜像是当前官方形态：`/dev/vda2` 为 `disk.img` 中的 VFAT 辅助分区。basic 源码中挂载目标是 `/dev/vda2` 到 `./mnt`，本次运行已实测 `mount/umount` 输出 success。

当前没有在 basic 中确认硬缺口的子项；主要问题是运行速度和 ext4 写路径放大。

补充细节：

- basic 小程序使用项目自带 ulib wrapper，不应只按普通 Linux libc 行为推断。
- `fstat` 在 basic ulib 中会走 `statx(fd, "", AT_EMPTY_PATH, ...)` fallback；当前内核有 `statx`，所以 `fstat` 不属于硬缺口。
- `gettimeofday`、`times`、`sched_yield` 当前已有 syscall 分发表项；本次已实测 `gettimeofday/sleep/times` 有正常输出，`yield` 因 timeout 尚未跑到。

## 顶层 suite 状态（按测试题可跑通程度排序）

这里的“支持”按比赛测试题口径记录：重点看当前脚本能不能启动、能不能跑到它自己的 success/结果输出、预计能通过多少子项；不等同于内核已经完整支持对应 Linux 功能。

支持最多的几个按当前静态检查应是：

1. `basic_testcode.sh`：子项最多能直接对上；本次已实测官方 RISC-V 形态可跑到 `unlink`。
2. `busybox_testcode.sh`：很多命令是基础文件/进程/文本处理，且脚本逐项打印结果。
3. `lua_testcode.sh`：解释器和简单脚本面较窄，预计比大型 benchmark 更容易跑出成功项。
4. `iperf_testcode.sh`：网络 syscall 面不是空的，`iperf3` 脚本也在顶层；它排在网络类最前，但六个网络小项不应静态认定全过。

| 排名 | suite | 测试题口径静态判断 |
| --- | --- | --- |
| 1 | `basic_testcode.sh` | 支持最多。300 秒官方 RISC-V 形态已实测跑过 `brk` 到 `uname`，进入 `unlink` 后 timeout。当前主要问题不是 syscall 缺口，而是直接在官方 ext4 测试盘上执行写入/删除导致运行过慢。 |
| 2 | `busybox_testcode.sh` | 测试题层面较高。脚本逐条读取 `busybox_cmd.txt` 并打印 `testcase busybox ... success/fail`，不因单项失败终止；`true/false/echo/pwd/uname/printf/ls/touch/cat/head/tail/rm/mkdir/mv/rmdir/cp/grep/find` 等基础项更有机会通过。`hwclock/dmesg/df/du/free/ps/uptime` 这类依赖设备、procfs 或更完整内核统计的项风险更高。 |
| 3 | `lua_testcode.sh` | 测试题层面中等偏高。`test.sh` 逐个运行 `date.lua/file_io.lua/max_min.lua/random.lua/remove.lua/round_num.lua/sin30.lua/sort.lua/strings.lua` 并打印 success/fail；简单数学、字符串、文件项有机会过。注意 `sort.lua` 使用 table 引用比较，`strings.lua` 的 `string.upper` 断言也有脚本自身逻辑风险，所以不能按全过记录。 |
| 4 | `iperf_testcode.sh` | 需要显式纳入：它就在顶层 musl suite。测试脚本启动 `iperf3 -s -D` 后跑 `BASIC_UDP/BASIC_TCP/PARALLEL_UDP/PARALLEL_TCP/REVERSE_UDP/REVERSE_TCP` 六项。内核已有 `socket/bind/listen/accept/connect/sendto/recvfrom/setsockopt/getsockopt/shutdown` 和 socket poll/read/write 路径，所以比纯空缺网络要强；但 parallel、reverse、UDP/TCP loopback、daemon/nonblock/timer 任一处不稳都会造成子项 fail。 |
| 5 | `netperf_testcode.sh` | 网络类第二梯队。脚本启动 `netserver`，再跑 `UDP_STREAM/TCP_STREAM/UDP_RR/TCP_RR/TCP_CRR`。和 iperf 依赖面相近，但多了 netserver daemon、RR/CRR 连接往返和更复杂参数，静态上比 iperf 更难全过。 |
| 6 | `iozone_testcode.sh` | 文件 IO 测试题有一定支撑，但压力大。脚本覆盖自动测量、4 线程吞吐、random/read-backwards/stride、stdio、pread/pwrite、preadv/pwritev；内核已有对应基础入口，能跑出部分项的机会比大型 libc/系统 benchmark 高，但线程、同步和文件系统稳定性决定能否连续通过。 |
| 7 | `libcbench_testcode.sh` | 单二进制，入口简单，但内部覆盖 pthread/malloc/fork/clock/proc 统计；能启动不代表能完整产出有效结果。读取 `/proc/self/smaps` 等点与当前简化 procfs 不匹配，按测试题口径放在中后段。 |
| 8 | `unixbench_testcode.sh` | 可能有少量纯计算项能跑出数字，但整套脚本串联 `dhry/whetstone/syscall/context/pipe/spawn/execl/fstime/shell/arith` 等，并大量依赖 busybox 管道过滤输出。测试题可通过面积小于前几项。 |
| 9 | `lmbench_testcode.sh` | 覆盖 syscall、select、signal、pipe、fork/exec/shell、文件带宽、pagefault、mmap、ctx switch。基础 syscall 项也许能跑，但 signal/select/context switch 等风险大，整体排序靠后。 |
| 10 | `libctest_testcode.sh` | 覆盖面太宽。`run-static.sh` 和 `run-dynamic.sh` 触及大量 libc、pthread、TLS、dlopen、sem、socket、time 测试；当前仍缺 `getrusage/clock_nanosleep/socketpair/sendmsg/recvmsg` 等，按“测试题全套可跑通”口径不应靠前。 |
| 11 | `cyclictest_testcode.sh` | 基本不支持。测试题需要实时调度、优先级、affinity、clock_nanosleep、pthread，并启动 `hackbench` 压力；即使脚本能进组，也很难获得有效 success。 |
| 12 | `ltp_testcode.sh` | 最不适合作为当前可跑通 suite。`ltp/testcases/bin` 下约 2820 个 case，覆盖 Linux 大量 syscall、权限、cgroup、网络、块设备等；脚本还会在每个 case 后无条件打印 `FAIL LTP CASE name : ret`，所以它本身也不是干净的 pass/fail 聚合。 |

## 当前 syscall 支撑面摘要

当前内核已经实现并分发了不少基础 syscall：

- 文件/目录：`getcwd`、`openat`、`close`、`read`、`write`、`readv`、`writev`、`pread64`、`pwrite64`、`preadv`、`pwritev`、`getdents64`、`mkdirat`、`unlinkat`、`fstat`、`statx`、`statfs`、`fsync`、`fdatasync`、`sync`、`renameat2`
- 进程：`clone`、`execve`、`wait4`、`exit`、`exit_group`、`getpid`、`getppid`、`gettid`
- 内存：`brk`、`mmap`、`munmap`、`mprotect`
- 时间/信号：`nanosleep`、`clock_gettime`、`clock_getres`、`getitimer`、`setitimer`、`kill`、`rt_sigaction`、`rt_sigprocmask`、`rt_sigsuspend` 等
- 网络：`socket`、`bind`、`listen`、`accept/accept4`、`connect`、`sendto`、`recvfrom`、`setsockopt`、`getsockopt`、`shutdown`，以及部分 socket `read/write/poll` 路径

明显影响测试套件的缺口包括：

- `clock_nanosleep`
- `socketpair`
- `sendmsg`
- `recvmsg`
- `getrusage`
- `clone3`
- `msync/mremap/madvise` 等更完整 mmap 周边
- 大量 LTP 需要的权限、cgroup、key、aio、xattr、namespace、module、bpf 等 Linux 面

## 建议的当前测试题优先级

如果目标是比赛测试题能跑出更多有效 success/结果输出，优先级建议和上表一致：

1. 先看 `basic_testcode.sh`、`busybox_testcode.sh`、`lua_testcode.sh`。
2. 然后看网络题 `iperf_testcode.sh`，再看 `netperf_testcode.sh`。
3. 文件 IO 压测 `iozone_testcode.sh` 放在网络之后。
4. `libcbench/unixbench/lmbench/libctest` 更适合当作中后期补面。
5. `cyclictest/ltp` 当前不适合当作近期可跑通目标。

如果只是想验证内核基础功能，而不是追测试题输出，不建议直接跑 `/tests/musl` 全量顶层脚本。更合理的最小功能验证集合是：

1. 只跑 `basic_testcode.sh`。
2. 若只做快速烟测，可先临时剔除 `unlink` 之后的子项，避免 ext4 写路径慢点遮挡前面结果。
3. 若确认使用分区盘并有 `/dev/vda2` VFAT，再保留 `mount/umount/openat` 这一组；否则先剔除 `mount/umount`。

全量 suite 当前更适合当作“测试题覆盖缺口雷达”，不适合作为严格 pass/fail 回归标准。

## 本次 ext4 写路径优化记录

2026-06-22 对 `os/src/fs/ext4/adpaters.rs` 做了低风险优化：

- `read_offset` 在 sector 对齐时直接返回底层 buffer，减少一次 `to_vec` 拷贝。
- `write_offset` 对完整覆盖的 512B sector 直接调用 `write_block`。
- 只有首尾不完整覆盖的 sector 保留 read-modify-write。
- 空写直接返回。

该优化减少 adapter 层的无意义读改写，但本次 300 秒官方 RISC-V 形态仍在 `test_unlink` timeout，说明瓶颈还包括 `ext4_rs` 内部写前读块、写后读回校验、以及 unlink/bitmap/writeback 元数据同步放大。


## 运行指令
可以，建议分三步执行，便于定位问题。

  1. 先构建：

  docker run --rm -v "$PWD":/work -w /work zhouzhouyi/os-contest:20260510 bash -lc 'make all'

  2. 确认产物存在：

  ls -lh kernel-rv kernel-la disk.img disk-la.img sdcard-rv.img

  3. 再单独跑 RISC-V 官方形态 QEMU：

  docker run --rm -it -v "$PWD":/work -w /work zhouzhouyi/os-contest:20260510 bash

  进容器后执行：

  qemu-system-riscv64 -machine virt -kernel kernel-rv -m 4G -nographic -smp 1 -bios default \
    -drive file=sdcard-rv.img,if=none,format=raw,id=x0 \
    -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 \
    -no-reboot -device virtio-net-device,netdev=net -netdev user,id=net \
    -rtc base=utc \
    -drive file=disk.img,if=none,format=raw,id=x1 \
    -device virtio-blk-device,drive=x1,bus=virtio-mmio-bus.1

  如果怕卡住，可以在 QEMU 命令前加：

  timeout --foreground 180s

  也就是：

  timeout --foreground 180s qemu-system-riscv64 ...
