# Trap 处理设计

本文记录异常,中断和系统调用的跨架构设计.具体异常号,寄存器字段和汇编槽位以源码为准.

## 当前状态

RISC-V 和 LoongArch 都提供 `arch::trap` 模块, 对通用内核暴露初始化,恢复,信号 trampoline 和 `TrapFrame` 操作.trap 汇编入口保存现场后进入 Rust handler, handler 分派 syscall,timer,IPI 或设备中断, 最终恢复当前任务的 `TrapFrame`.

RISC-V 路径已覆盖 syscall,timer,software interrupt IPI 和 external interrupt.LoongArch 路径已覆盖 syscall,timer,TLB refill 入口安装和基本恢复, 外部中断/IPI 仍未与 RISC-V 对齐.

## 目标和非目标

目标:

- 在架构层封装 trap entry 和 trap return.
- 让 syscall 分派,计时器队列和调度器复用通用内核逻辑.
- 支持 trap handler 内发生调度后恢复新任务上下文.
- 保持信号返回 trampoline 的架构字节由各架构提供.

非目标:

- 不在正式文档中维护完整异常码表.
- 不把硬中断处理写成可阻塞路径.
- 不把用户态所有异常都立即实现为完整 POSIX 信号语义.

## 关键流程

```text
hardware trap
  -> arch trap_entry
  -> save TrapFrame
  -> Rust trap_handler
  -> syscall/timer/IPI/device/exception dispatch
  -> maybe schedule
  -> restore current task TrapFrame
  -> sret/ertn
```

系统调用会调整返回 PC, 然后把 `TrapFrame` 交给 syscall dispatcher.时钟中断推进全局 ticks,timer queue 和 interval timer, 必要时触发调度.RISC-V 软件中断先处理 IPI pending 标志, 再根据 run queue 判断是否调度.

## 用户态和内核态异常

用户态异常不应直接破坏内核.RISC-V 当前会打印诊断信息并终止当前任务, 后续可进一步映射为 SIGILL/SIGSEGV 等信号.LoongArch 当前用户态未知异常仍偏 bringup 诊断, 会 panic, 这是需要继续收敛的限制.

内核态异常按致命错误处理, 会打印关键寄存器并 panic.

## 并发和生命周期约束

- trap handler 入口时中断通常已关闭, 返回必须通过架构 restore 恢复特权状态和中断状态.
- trap handler 中不要执行可能阻塞或长期持锁的工作; 网络轮询等工作应转交 kworker.
- timer 中断唤醒任务时依赖调度器 wakeup 幂等性.
- 如果 trap handler 中发生任务切换, 返回必须读取当前任务的 `trap_frame_ptr`.
- `TrapFrame` 布局必须和汇编保存/恢复严格一致.

## 已知限制

- LoongArch 外部中断和 IPI 尚未接入完整分派.
- LoongArch 用户态未知异常还没有按 RISC-V 方式转换为任务终止或信号.
- RISC-V TLB flush IPI 当前处理本地 `sfence.vma`, 更强同步协议需要内存管理侧补足.

## 源码索引

- `os/src/arch/riscv/trap/mod.rs`: RISC-V trap 初始化和恢复门面.
- `os/src/arch/riscv/trap/trap_handler.rs`: RISC-V syscall/timer/IPI/device/异常分派.
- `os/src/arch/riscv/trap/trap_entry.S`: RISC-V 保存和恢复汇编.
- `os/src/arch/loongarch/trap/mod.rs`: LoongArch trap 初始化和恢复门面.
- `os/src/arch/loongarch/trap/trap_handler.rs`: LoongArch syscall/timer/TLB refill 入口安装和异常分派.
- `os/src/arch/loongarch/trap/trap_entry.S`: LoongArch 保存,恢复和 TLB refill 汇编.
