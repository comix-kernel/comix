# 调度器设计

## 当前状态

调度器采用 per-CPU `RRScheduler`.每个 CPU 有独立 run queue 和 idle task, 新任务或被唤醒任务按 affinity mask 选择目标 CPU.RISC-V 跨核唤醒会发送 reschedule IPI; LoongArch 当前单核 IPI 为 no-op.

策略上仍是简单 RR, 但 run queue 弹出时会优先选择更高 `sched_priority` 的任务, 同优先级保持队列相对顺序.

## 目标和非目标

目标:

- 在每个 CPU 上维护独立运行队列, 减少全局调度锁.
- 保证 wakeup 幂等, 避免同一任务同时进入多个 CPU 的 run queue.
- 用统一 `Scheduler` trait 隔离调度策略和外部任务生命周期调用.
- 在没有可运行任务时切到本 CPU idle task.

非目标:

- 不实现完整 CFS/RT 调度语义.
- 不实现通用内核抢占模型.
- 不在调度器里负责进程资源释放或 wait 语义.

## 模块边界

- `scheduler/mod.rs`: per-CPU 调度器数组,CPU 选择,公共 sleep/wake/exit/schedule 接口.
- `rr_scheduler.rs`: run queue 选择,时间片,上下文切换计划.
- `task_queue.rs`: 基于 `SharedTask` 身份的队列容器.
- `wait_queue.rs`: 事件等待队列, 调用调度器完成睡眠和唤醒.
- `kernel/cpu.rs`: 当前任务,当前地址空间和 idle task 切换.

## 关键流程

### schedule

```text
disable interrupts
  -> if current task can keep running and run queue empty, return
  -> lock current CPU scheduler
  -> choose next task or idle
  -> current_cpu.switch_task
  -> build SwitchPlan
  -> unlock scheduler
  -> arch context_switch
restore interrupt state
```

调度器锁不覆盖汇编切换本身.`next_task` 生成旧/新 `Context` 指针, 真正保存恢复由架构 `context_switch` 完成.

### sleep

sleep 只改变任务状态并从所属 CPU run queue 移除, 不隐式切换.调用方通常随后调用 `schedule` 或在当前路径返回到可调度点.

`sleep_task_prepare` 把条件检查和睡眠状态转换放在调度器锁内, 用于避免 lost wakeup.

### wake

唤醒先按任务 affinity 和在线 CPU mask 选择目标 CPU.随后在目标 CPU 调度器锁下持有任务锁, 如果任务已经是 `Running`,`Zombie` 或 `Stopped`, 直接返回; 否则设置 `Running`,更新 `on_cpu` 并入队.目标 CPU 不是当前 CPU 时发送 reschedule IPI.

## 并发和生命周期约束

- 调度入口会禁用中断并在返回时恢复原状态.
- `current_cpu().switch_task` 会切换用户地址空间并更新 `TrapFrame.cpu_ptr`.
- wakeup 必须以任务状态为幂等屏障, 防止同一任务被两个 CPU 同时运行.
- idle task 不在普通 run queue 中, 只作为空队列兜底.
- run queue 内的身份判断基于 `Arc::ptr_eq`, 不是 tid 值.

## 已知限制

- 时间片默认很小, 当前用于可用性而非性能调优.
- `sched_policy` 字段存在, 但完整 Linux 调度策略尚未实现.
- LoongArch 暂无跨核唤醒能力, 因此 per-CPU 设计在该架构上仍以单核方式运行.

## 源码索引

- `os/src/kernel/scheduler/mod.rs`: per-CPU 调度器,CPU 选择,sleep/wake/schedule.
- `os/src/kernel/scheduler/rr_scheduler.rs`: RR 策略,idle fallback 和 `SwitchPlan` 创建.
- `os/src/kernel/scheduler/task_queue.rs`: run queue 容器.
- `os/src/kernel/scheduler/wait_queue.rs`: wait queue 与调度器交互.
- `os/src/kernel/cpu.rs`: `switch_task`,地址空间切换和 idle task.
