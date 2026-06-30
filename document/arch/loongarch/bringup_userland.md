# LoongArch64 用户态 bringup 设计记录

本文保留 LoongArch64 能进入用户态所依赖的设计点, 不再作为逐项修复流水账维护.

## 当前状态

LoongArch64 已能通过公共启动流创建 PID 1, 执行 `/sbin/init`, 并依赖用户态 rcS 完成 `/dev`,`/proc`,`/sys`,`/tmp` 等挂载.内核侧在 mount 特殊路径中提供必要兜底, 让 rootfs 缺少挂载点目录时仍能继续启动.

用户态运行依赖四个架构点:

- trap entry 和 `TrapFrame` 布局必须严格一致.
- TLB refill 入口不能破坏 trap entry 依赖的 scratch/CSR 状态.
- exec 栈布局必须满足 LoongArch Linux ABI 对 argv/envp/auxv/TLS 的基本要求.
- 启动早期必须使能基础浮点指令, 避免用户程序早期 FP 指令异常.

## 目标和非目标

目标:

- 让 LoongArch 与 RISC-V 在通用启动语义上保持一致.
- 把用户态 ABI 差异限制在 `arch/loongarch` 的 trap,task 和 mm 代码中.
- 保留定位用户态早期异常所需的诊断线索.

非目标:

- 不长期保留每个历史 bug 的详细分支和补丁说明.
- 不在架构文档中描述 VFS mount 的完整实现.
- 不把当前调试日志数量视为稳定行为.

## 关键设计点

### TrapFrame 一致性

LoongArch 用户态寄存器污染通常会表现为 `BADV` 指向内核高地址.此类问题优先检查 `trap_entry.S` 保存/恢复顺序与 `trap_frame.rs` 布局是否一致, 尤其是 callee-saved 寄存器和 `$tp/$sp/$ra`.

### TLB refill

TLB refill 使用独立入口, 通过软件页表遍历填充 TLB.该路径必须避免覆盖 trap entry 用来定位保存区的 scratch 状态, 否则会出现 trap storm 或恢复到错误现场.

### 用户栈和 TLS

exec 路径构造 argv/envp/auxv, 并为 LoongArch 设置 TLS/thread pointer.用户程序进入 libc 早期初始化前, `$sp`,`$tp` 和参数寄存器必须同时满足 ABI 预期.

### 启动职责

内核负责发现并挂载根文件系统, `/dev` 等运行期伪文件系统交给 rcS.内核保留挂载点兜底创建, 这是为了兼容不同架构 rootfs 镜像内容差异.

## 已知限制

- 用户态异常处理仍偏 bringup 诊断, 还没有完整映射到 POSIX 信号.
- 外部中断和多核 IPI 未完成.
- rootfs 目录结构最好继续向 RISC-V 镜像对齐, 减少内核兜底逻辑的必要性.

## 源码索引

- `os/src/arch/loongarch/trap/trap_entry.S`: trap 保存恢复和 TLB refill.
- `os/src/arch/loongarch/trap/trap_frame.rs`: LoongArch `TrapFrame` 和 exec 返回现场.
- `os/src/arch/loongarch/trap/trap_handler.rs`: syscall/timer/异常分派和入口安装.
- `os/src/arch/loongarch/kernel/task.rs`: 用户栈和 TLS 布局.
- `os/src/arch/loongarch/boot/mod.rs`: 基础 FPU 使能和公共启动入口.
