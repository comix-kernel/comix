# LoongArch64 架构设计

LoongArch64 当前文档聚焦启动,trap 和用户态运行的设计边界.历史 bringup 复盘见 `bringup_userland.md`, 当前架构总览见 `../README.md`.

## 当前状态

LoongArch64 已接入公共启动流,用户态 exec 栈构造,trap 保存恢复,timer 中断和 TLB refill 入口安装.启动阶段只通过 `PrimaryBootOps` 挂接基础 FPU 使能, 其余 init/idle/rootfs 逻辑复用 `kernel::boot`.

当前仍是单核架构路径.IPI 接口保留为 no-op 兼容层, 让通用调度代码不需要为 LoongArch 特判.

## 目标和非目标

目标:

- 与 RISC-V 共享公共启动,任务和调度模型.
- 在 `arch/loongarch` 内封装 CSR,DMW,TLB refill 和 trap entry 差异.
- 保持用户态 busybox/init 类程序可通过 Linux ABI 子集运行.

非目标:

- 不承诺 LoongArch 与 RISC-V 当前具备相同 SMP 能力.
- 不在文档中维护 CSR 编号和 TrapFrame 槽位清单.
- 不把 bringup 调试日志视为长期接口.

## 关键流程

启动:

```text
entry.S -> loongarch::boot::main -> enable_base_fp -> kernel::boot::run_primary_boot
```

trap:

```text
trap_entry -> trap_handler -> syscall/timer/exception -> restore current TrapFrame
```

TLB refill 入口由 trap 初始化阶段写入 CSR, 使用独立汇编路径完成软件页表遍历和 `tlbfill`.

## 已知限制

- IPI 和 SMP 启动尚未实现.
- 外部中断分派还未与 RISC-V 对齐.
- 用户态未知异常当前仍是诊断/panic 路径, 后续应收敛为任务终止或信号.

## 源码索引

- `os/src/arch/loongarch/boot/mod.rs`: 主核入口和基础 FPU 使能.
- `os/src/arch/loongarch/trap/mod.rs`: trap 门面.
- `os/src/arch/loongarch/trap/trap_handler.rs`: trap 分派和 TLB refill 入口安装.
- `os/src/arch/loongarch/trap/trap_entry.S`: 保存恢复和 TLB refill 汇编.
- `os/src/arch/loongarch/mm/mod.rs`: 直接映射窗口和内核根页表记录.
- `os/src/arch/loongarch/ipi.rs`: 单核 IPI 兼容接口.
