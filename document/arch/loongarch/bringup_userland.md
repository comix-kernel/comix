# LoongArch64 启动与用户态运行修复总结（comix-1 当前分支）

本文档总结当前分支中，为了让 comix 在 LoongArch64 架构下能够**稳定完成启动、进入并持续运行用户态程序（busybox init / shell）**而做的一组修复与对齐工作。重点目标是：让 LoongArch 的整体启动语义尽可能与 RISC-V 路径一致（尤其是 `/dev` 等挂载点由 rcS 负责挂载），并且把“卡在 trap / 无法进入用户态 / 用户态一运行就异常”的问题收敛到可解释、可复现、可继续演进的状态。

---

## 1. 背景与目标

### 1.1 现象概述（修复前）

LoongArch64 上曾出现以下典型现象（可单独出现或互相叠加）：

- `kernel_execve("/sbin/init")` 之后无法稳定进入用户态，串口输出停止或在 GDB 中反复停在 `trap_entry`。
- 即使偶尔能进入用户态执行少量 syscall，也很快出现用户态异常：
  - `estat` 指示地址相关异常（如地址错、权限错等），
  - `badv` 出现 `0x9000...` 这一类**内核高地址**，明显是用户态不应访问的区域。
- init 进程行为异常（例如 busybox 提示必须是 PID 1，或内核侧找不到 init 进程）。
- `/dev/ttyS0` 等设备节点缺失，导致 rcS 无法起交互 shell。

### 1.2 修复目标

- **可靠进入用户态并持续运行**：用户态能完成 `init -> rcS -> spawn shell`，至少出现 `/ #` 提示符。
- **启动流程对齐**：LoongArch 与 RISC-V 的职责边界一致：
  - 内核负责挂载根文件系统（Ext4 root）。
  - `/dev /proc /sys /tmp` 等由用户态 rcS 执行 `mount` 完成。
  - 内核在 `mount("/dev")` 时做必要的内核侧初始化（创建设备节点）。
- **可调试性**：提供足够的日志/断点点位，能明确区分：
  - trap 入口保存/恢复错误
  - TLB refill 错误
  - 用户栈/TLS 约定问题
  - rootfs/挂载点目录缺失等“系统集成”问题

---

## 2. 关键问题定位方法（建议保留）

### 2.1 典型 GDB/寄存器观察点

当卡在 `trap_entry` 或用户态异常时，优先观察：

- `ESTAT/ERA/BADV/BADI`：
  - `ESTAT` 的 `ecode/esubcode` 用来区分 syscall / page fault / 地址错 / 指令错。
  - `ERA` 是异常发生时的 PC（用户态异常时通常在 `0x12...` 这类用户映射区）。
  - `BADV` 是访问地址（若出现在 `0x9000...`，高度怀疑寄存器现场被内核保存/恢复逻辑污染）。
  - `BADI` 是故障指令（可用来对照用户 ELF 的指令流）。

### 2.2 “用户态 BADV 指向 0x9000...” 的含义

这是本次最有信息量的信号之一：用户程序在执行正常指令时，某个寄存器被污染成内核高地址（或某个指针被写错），从而产生“用户态非法访问内核区”。这种现象在 LoongArch 上最常见的根因是：

- trap 入口保存现场写错槽位（TrapFrame 布局与汇编保存顺序不一致），或
- trap 返回恢复现场读错槽位，导致用户寄存器恢复后被污染。

---

## 3. 修复项总览（按子系统）

### 3.1 启动入口与 DTB（设备树）指针探测

**问题**：QEMU/固件传入 DTB 指针的寄存器约定不稳定；直接使用错误指针会导致 FDT 解析失败，影响设备初始化。

**修复**：在汇编入口 `_start` 中对候选 DTB 地址做 magic 检查，选择有效指针后写入全局 `DTP`（保存物理地址，由 Rust 侧再转换直映虚拟地址）。

- 相关文件：
  - `os/src/arch/loongarch/boot/entry.S`

### 3.2 TLB Refill：可用的 refill 入口与“不要踩 CSR”

**问题**：用户态运行时大量缺页/首次访问需要 TLB refill；若 refill 入口不正确或破坏了 trap 入口依赖的 CSR/寄存器，会表现为随机 trap storm 或直接卡死。

**修复**：

- 使用 `lddir/ldpte/tlbfill` 的硬件辅助页表遍历路径实现 `tlb_refill_entry`。
- refill 入口只使用 `CSR 0x8b (TLBRSAVE)` 保存/恢复 `$t0`，避免覆写 `KSAVE/KSCRATCH` 一类 CSR，从而不干扰 `trap_entry` 的 TrapFrame 指针机制。

- 相关文件：
  - `os/src/arch/loongarch/trap/trap_entry.S`

### 3.3 trap_entry / __restore：寄存器现场保存/恢复一致性（核心）

**问题**：TrapFrame 与汇编保存/恢复顺序不一致会直接导致：

- 用户态 syscall 返回后寄存器被污染（例如 s0..s8 偏移错一位），
- 进而出现用户态 `BADV=0x9000...` 的非法访存异常。

**修复**：

- trap 入口先把原始 `$a0/$t1` 写入 scratch CSR，再读取 `KScratch0` 中的 TrapFrame 指针；
- 保存/恢复时严格按 LoongArch ABI 寄存器编号对应 TrapFrame 槽位；
- 修正了 callee-saved 区（`$s0..$s8`）的槽位映射，避免“漏存 $s0 导致整段错位”。

- 相关文件：
  - `os/src/arch/loongarch/trap/trap_entry.S`
  - `os/src/arch/loongarch/trap/trap_frame.rs`（TrapFrame 结构与 exec 初始化）
  - `os/src/arch/loongarch/trap/trap_handler.rs`（用户态异常打印、syscall 分发与 restore）

### 3.4 用户态 TLS/TP 与用户栈布局（musl/busybox 依赖）

**问题**：许多 LoongArch Linux-ABI 用户程序依赖 `$tp` 作为 TLS 指针；如果 execve 后 `$tp` 没有按约定设置，早期 libc 初始化可能会崩。

**修复**：

- 在用户栈顶预留一页作为 TLS/TCB 区域；
- 将 `$tp` 设置为该页内一个稳定的对齐地址，并写入最小 self-pointer（满足常见 libc 期望）；
- 在用户栈中构造 `argc/argv/envp/auxv`（包含 `AT_PHDR/AT_ENTRY/AT_PLATFORM/AT_RANDOM/AT_EXECFN` 等）；
- `set_exec_trap_frame()` 将 `tp/sp/a0/a1/a2` 等寄存器按 LoongArch ABI 写入 TrapFrame。

- 相关文件：
  - `os/src/arch/loongarch/kernel/task.rs`（`setup_stack_layout()`）
  - `os/src/kernel/task/task_struct.rs`（execve 路径调用与写回）
  - `os/src/arch/loongarch/trap/trap_frame.rs`

### 3.5 FPU 使能（EUEN.FPE）

**问题**：部分用户程序会在非常早的阶段执行 FP 指令；若未启用基础 FPU，会出现异常或非预期行为。

**修复**：在 LoongArch 启动早期设置 `EUEN.FPE = 1`。

- 相关文件：
  - `os/src/arch/loongarch/boot/mod.rs`

### 3.6 启动流程与任务模型对齐（init PID=1、idle_task）

**问题**：

- busybox init 要求自己是 PID 1；如果内核创建的 init task 不是 TID/PID 1，会直接报错。
- 调度器在 runqueue 为空时会切换到每 CPU 的 idle_task；若未设置，会触发 panic。

**修复**：

- LoongArch `rest_init()` 固定创建 init 任务为 `tid=pid=1`（与 RISC-V 一致）。
- 在 CPU0 预先创建并登记 idle 任务（不加入 runqueue，仅作为兜底）。

- 相关文件：
  - `os/src/arch/loongarch/boot/mod.rs`

### 3.7 `/dev` 挂载与设备节点：让 LoongArch 与 RISC-V 行为一致

**问题根因**（为什么 RISC-V 没问题而 LoongArch 出问题）：

- 运行时镜像由 `os/build.rs` 从 `data/<arch>_musl` 目录生成：
  - `fs-riscv.img` 根目录自带 `/dev /proc /sys` 等目录；
  - `fs-loongarch.img` 根目录不包含 `/dev`（也可能不包含 `/proc /sys`）。
- RISC-V 路径把 `/dev` 挂载留给 rcS，并依赖内核对 `mount("/dev")` 的特殊处理自动 `init_dev()`；因为 `/dev` 目录存在，所以工作正常。
- LoongArch 若沿用“rcS 挂载 /dev”，但镜像里没有 `/dev` 目录，则挂载点不存在，后续 `init_dev()` 无法创建 `/dev/ttyS0`，最终出现 `can't open /dev/ttyS0`。

**修复策略**：

1) **启动流程对齐**：LoongArch 与 RISC-V 一样，把 `/dev(/proc,/sys,/tmp)` 的挂载交给用户态 rcS 完成。

- `os/src/arch/loongarch/boot/mod.rs`
- `data/loongarch_musl/etc/init.d/rcS`

2) **内核兜底**：在 `SYS_MOUNT` 的特殊分支中，为 `/dev /proc /sys /tmp` 增加“挂载点目录不存在则先创建”的逻辑，然后再执行挂载动作；`/dev` 分支继续在挂载 tmpfs 后调用 `init_dev()` 创建设备节点。

- `os/src/kernel/syscall/fs.rs`

> 这使得：即使某个架构的 rootfs 镜像缺少挂载点目录，rcS 的挂载也不会因为“目录不存在”而失败，从而让启动语义更稳健、更一致。

---

## 4. 启动流程对齐总结（RISC-V vs LoongArch）

对齐后的关键点：

- 两个架构都在内核 init task 中挂载/初始化 Ext4 root（作为根文件系统）。
- `/dev /proc /sys /tmp` 均由用户态 rcS 执行 `mount` 完成。
- 内核在 `SYS_MOUNT` 中对这些 target 做特殊处理：
  - `/dev`：挂载 tmpfs 后自动 `init_dev()` 创建设备节点；
  - `/proc`：`init_procfs()`；
  - `/sys`：`init_sysfs()`；
  - `/tmp`：`mount_tmpfs()`；
  - 并在进入这些特殊处理前确保挂载点目录存在。

---

## 5. 验证结果（当前分支）

### 5.1 运行验证

使用：

- `make run ARCH=loongarch`

预期能观察到：

- `/sbin/init` 启动并执行 rcS，
- 后续出现 shell 提示符（例如 `/ #`），说明用户态已经稳定运行并能进行基本交互。

### 5.2 验证点解释

- 能连续看到大量 syscall（包含 `mount`、`openat`、`read/write`、`clone` 等）且不再出现用户态 `BADV=0x9000...` 类型的地址异常，说明 trap 保存/恢复与 TLB refill 已经基本稳定。
- shell 能起则说明 `/dev/ttyS0` 已存在且可打开（这依赖 rcS mount("/dev") + 内核 init_dev()）。

---

## 6. 后续建议（非阻塞）

1) **rootfs 内容对齐**：建议让 `data/loongarch_musl` 也包含 `/dev /proc /sys` 等空目录，使镜像结构更接近 `data/risc-v_musl`，减少对内核“兜底创建目录”的依赖。
2) **减少调试噪声**：目前 LoongArch trap/syscall 输出较多（用于 bringup）；在稳定后可逐步降级为 `pr_debug` 或加开关。
3) **设备/网络完善**：virtio-net 可能仍会报 `DeviceNotReady`，这属于设备初始化/驱动完善方向，与“能进用户态”已解耦。

