# 调度器设计

## 当前状态

调度器类型仍叫 `RRScheduler`, 但每个 CPU 上已经分成两条运行队列:

- 实时队列: `SCHED_FIFO` 与 `SCHED_RR`, 且 `sched_priority > 0`.
- fair 队列: `SCHED_NORMAL`, `SCHED_BATCH`, `SCHED_IDLE`. 优先级不大于 0 的实时任务, 以及未识别策略, 也进入这条队列.

选择下一个任务时先取实时队列里 `sched_priority` 最高的任务; 实时队列为空时, 再取 fair 队列里 `vruntime` 最小的任务. 相同 `vruntime` 保持入队顺序.

每个 CPU 有独立的这两条队列和自己的 idle task. 新任务或被唤醒任务按 affinity mask 选择目标 CPU. RISC-V 跨核唤醒会发送 reschedule IPI; LoongArch 当前单核, IPI 为 no-op.

时钟滴答会给当前 fair 任务增加 `vruntime`: `SCHED_NORMAL` 和 `SCHED_BATCH` 加 1, `SCHED_IDLE` 加 8, 实时任务不加. `SCHED_FIFO` 不因普通滴答轮转. `SCHED_RR` 使用固定实时时间片, fair 类时间片按本 CPU fair 队列长度在目标延迟和该类最小粒度之间取值. 队列里出现更高优先级的实时任务时, 当前任务会被抢占.

fair 任务入队前, 如果它的 `vruntime` 小于队列中的最小值, 会被抬到这个最小值, 避免睡眠后回来的任务长期占住 CPU.

## 目标和非目标

目标:

- 在每个 CPU 上维护独立运行队列, 减少全局调度锁.
- 让实时任务优先于 fair 任务, 并在实时任务之间遵守优先级.
- 用 `vruntime` 在 fair 类内部近似公平, 并用随队列长度变化的时间片限制单次运行.
- 保证 wakeup 幂等, 避免同一任务同时进入多个 CPU 的 run queue.
- 用统一 `Scheduler` trait 隔离调度策略和外部任务生命周期调用.
- 在没有可运行任务时切到本 CPU idle task.

非目标:

- 不实现 Linux CFS 的权重, 组调度, 带宽控制或跨 CPU `vruntime` 负载均衡.
- 不实现通用内核抢占. 当前只在时间片耗尽或更高优先级实时任务出现时重新选择任务.
- 不在调度器里负责进程资源释放或 wait 语义.

## 模块边界

- `scheduler/mod.rs`: per-CPU 调度器数组, CPU 选择, 公共 sleep/wake/exit/schedule 接口.
- `rr_scheduler.rs`: 实时/fair 队列, 时间片, `vruntime` 记账和上下文切换计划.
- `task_queue.rs`: 基于 `SharedTask` 身份的队列容器, 以及按优先级或 `vruntime` 取出任务.
- `wait_queue.rs`: 事件等待队列, 调用调度器完成睡眠和唤醒.
- `kernel/cpu.rs`: 当前任务, 当前地址空间和 idle task 切换.

## 关键流程

### schedule

```text
disable interrupts
  -> if current task can keep running and run queue empty, return
  -> lock current CPU scheduler
  -> choose highest-priority realtime task, else minimum-vruntime fair task, else idle
  -> current_cpu.switch_task
  -> build SwitchPlan
  -> unlock scheduler
  -> arch context_switch
restore interrupt state
```

调度器锁不覆盖汇编切换本身. `next_task` 生成旧/新 `Context` 指针, 真正保存恢复由架构 `context_switch` 完成.

被换下且仍为 `Running` 的任务按它的 `sched_policy` 放回对应队列尾部. fair 任务下一次被选中仍取决于 `vruntime`, 不只取决于尾部位置.

### 时间片与抢占

```text
timer tick
  -> charge current fair task vruntime
  -> if a queued realtime task outranks current, reschedule
  -> SCHED_FIFO stays on CPU
  -> SCHED_RR and fair tasks decrement the current slice
  -> slice expiry reschedules and assigns a new slice
```

fair 时间片约为目标延迟除以当前 fair 可运行任务数, 并且不会小于该策略的最小粒度. `SCHED_RR` 使用单独的固定时间片.

### sleep

sleep 只改变任务状态并从所属 CPU 的两条队列中移除, 不隐式切换. 调用方通常随后调用 `schedule`, 或在当前路径返回到可调度点.

`sleep_task_prepare` 把条件检查和睡眠状态转换放在调度器锁内, 用于避免 lost wakeup.

### wake

唤醒先按任务 affinity 和在线 CPU mask 选择目标 CPU. 随后在目标 CPU 调度器锁下持有任务锁. 如果任务已经是 `Running`, `Zombie` 或 `Stopped`, 直接返回; 否则设置 `Running`, 更新 `on_cpu` 并入队. 入 fair 队列时会先钳制 `vruntime`. 目标 CPU 不是当前 CPU 时发送 reschedule IPI.

## 并发和生命周期约束

- 调度入口会禁用中断并在返回时恢复原状态.
- `current_cpu().switch_task` 会切换用户地址空间并更新 `TrapFrame.cpu_ptr`.
- wakeup 必须以任务状态为幂等屏障, 防止同一任务被两个 CPU 同时运行.
- idle task 不在普通 run queue 中, 只作为空队列兜底.
- run queue 内的身份判断基于 `Arc::ptr_eq`, 不是 tid 值.
- 同一个任务同一时刻只应出现在一个 CPU 的一条队列里. 移除任务时两条队列都会查.

## 已知限制

- fair 记账按固定滴答增量, 不读取 nice 或调度权重.
- 时间片和目标延迟是小整数滴答, 用于把 CPU 让出来, 不是性能调优后的延迟目标.
- 没有跨 CPU 迁移正在运行的任务, 也没有按 `vruntime` 平衡多个 CPU.
- `SCHED_FIFO` 只要没有更高优先级实时任务, 就可以一直占用当前 CPU.
- LoongArch 暂无跨核唤醒能力, 因此 per-CPU 设计在该架构上仍以单核方式运行.

## 源码索引

- `os/src/kernel/scheduler/mod.rs`: per-CPU 调度器, CPU 选择, sleep/wake/schedule.
- `os/src/kernel/scheduler/rr_scheduler.rs`: 双队列, 时间片, `vruntime` 和 `SwitchPlan`.
- `os/src/kernel/scheduler/task_queue.rs`: 队列容器, 最高优先级和最小 `vruntime` 选择.
- `os/src/kernel/scheduler/wait_queue.rs`: wait queue 与调度器交互.
- `os/src/kernel/cpu.rs`: `switch_task`, 地址空间切换和 idle task.
- `os/src/uapi/sched.rs`: `SCHED_*` 策略常量.
