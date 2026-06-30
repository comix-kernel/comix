# RISC-V IPI 设计

## 当前状态

RISC-V IPI 用于跨 CPU 通知.当前实现基于 SBI 发送软件中断, 并用 per-CPU 原子 pending 标志记录待处理动作.已接入的动作包括 reschedule,TLB flush 和 stop.

调度器唤醒任务时, 如果目标 CPU 不是当前 CPU, 会发送 reschedule IPI.接收 CPU 在软件中断 trap 中处理 pending 标志, 然后按 run queue 状态决定是否调度.

## 目标和非目标

目标:

- 在硬中断上下文中以无分配方式处理跨核通知.
- 支持多个 IPI 类型合并到同一次 pending 标志.
- 支持批量发送, 减少 SBI 调用.
- 为调度唤醒和 TLB shootdown 提供统一机制.

非目标:

- 不在 IPI 层等待远端 CPU 完成某项工作.
- 不在 IPI handler 中执行可阻塞操作.
- 不由 IPI 层决定调度策略.

## 关键流程

```text
sending CPU
  -> set IPI_PENDING[target] with Release
  -> SBI send_ipi

target CPU
  -> software interrupt trap
  -> clear SSIP
  -> swap pending with AcqRel
  -> handle reschedule/TLB flush/stop
  -> trap handler may schedule
```

reschedule IPI 只负责打断目标 CPU 并暴露"有调度事件"这一事实.是否真正切换任务, 由 trap handler 查看当前 CPU run queue 后决定.

## 并发约束

- 发送端先写 pending, 再触发 SBI IPI.
- 接收端用 `swap(0)` 一次性取走并清空 pending, 合并处理多个标志.
- IPI handler 不能分配内存, 不能持有会阻塞的锁.
- TLB flush IPI 当前只执行本地 `sfence.vma`; 更强完成确认应由调用方设计同步协议.

## 已知限制

- RISC-V hart id 当前按 CPU id 位图发送, 依赖平台启动时的 hart/CPU 映射保持一致.
- TLB shootdown 缺少远端完成 ack.
- stop IPI 进入等待中断循环, 没有完整关机协调协议.

## 源码索引

- `os/src/arch/riscv/ipi.rs`: IPI 类型,pending 标志,发送和处理.
- `os/src/arch/riscv/trap/trap_handler.rs`: 软件中断分派和调度触发.
- `os/src/kernel/scheduler/mod.rs`: 跨 CPU wakeup 发送 reschedule IPI.
- `os/src/arch/riscv/lib.rs`: SBI 调用封装.
