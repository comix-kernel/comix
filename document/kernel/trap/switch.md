# 上下文切换流程

## 当前状态

上下文切换由调度器和架构汇编共同完成.调度器决定下一个任务并更新 `current_cpu`, 架构 `context_switch` 保存旧 `Context`,恢复新 `Context`.如果切换发生在 trap handler 中, 最终 trap return 会恢复新任务的 `TrapFrame`.

## 目标和非目标

目标:

- 让调度策略不依赖具体寄存器布局.
- 在切换任务时同步地址空间和 `TrapFrame.cpu_ptr`.
- 让第一次调度,普通 yield,timer 抢占和 idle fallback 使用同一套切换机制.

非目标:

- 不在文档中逐条描述汇编保存指令.
- 不把 `TrapFrame` 当作普通调度上下文使用.
- 不在调度器锁内执行长期工作.

## 普通切换

```text
schedule
  -> scheduler.next_task
  -> current_cpu.switch_task(next)
  -> build old/new Context pointers
  -> arch context_switch(old, new)
  -> ret into next Context.ra
```

`current_cpu.switch_task` 对用户任务会激活其地址空间.对所有任务都会调用架构 hook 更新 trap frame 中保存的 CPU 指针, 这是多核迁移后 trap entry 恢复 per-CPU 指针的关键.

## 第一次切换

新任务的 `Context.ra` 指向 `forkret`.第一次恢复该上下文时, CPU 从 `forkret` 继续执行.`forkret` 不返回普通 Rust 调用栈, 而是调用架构恢复逻辑进入内核线程入口或用户态入口.

## trap 中切换

timer 或 IPI 进入 trap handler 后可能调用 `schedule`.此时入口传入的 `TrapFrame` 属于旧任务; 调度后当前任务可能已经变成另一个任务.因此 handler 尾部必须读取 `try_current_task().trap_frame_ptr` 并恢复该指针.

## idle fallback

每个 CPU 在启动时创建 idle task.run queue 为空且当前任务已阻塞或退出时, `RRScheduler` 切换到本 CPU idle task.idle task 不入普通 run queue, 也不参与公平性计算.

## 源码索引

- `os/src/kernel/scheduler/mod.rs`: `schedule` 和公共调度入口.
- `os/src/kernel/scheduler/rr_scheduler.rs`: `next_task`,idle fallback 和 `SwitchPlan`.
- `os/src/kernel/cpu.rs`: `switch_task`,地址空间切换和 `TrapFrame.cpu_ptr` 更新.
- `os/src/kernel/task/mod.rs`: `forkret`.
- `os/src/arch/riscv/kernel/switch.S`: RISC-V 切换汇编.
- `os/src/arch/loongarch/kernel/switch.S`: LoongArch 切换汇编.
