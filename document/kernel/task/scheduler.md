# 任务的调度

本文档阐述了 `comix` 内核中的任务调度机制，包括调度器设计、调度时机以及上下文切换流程。

## 1. 调度器设计 (`Scheduler` Trait)

为了实现可扩展和可替换的调度策略，我们抽象出了一个 `Scheduler` Trait。任何具体的调度器实现都必须实现这个 Trait 中定义的方法。

**源码链接**: [`os/src/kernel/scheduler/mod.rs`](/os/src/kernel/scheduler/mod.rs)

`Scheduler` Trait 定义了以下核心接口：

- `new() -> Self`: 创建一个新的调度器实例。
- `add_task(&mut self, task: SharedTask)`: 将一个新任务添加到调度器的运行队列中。
- `next_task(&mut self) -> Option<SharedTask>`: 从运行队列中选择下一个要执行的任务。
- `prepare_switch(&mut self) -> Option<SwitchPlan>`: 准备进行任务切换。这是调度的核心决策逻辑，它会选择下一个任务，并返回一个包含新旧任务上下文指针的 `SwitchPlan`。
- `sleep_task(&mut self, task: SharedTask, ...)`: 将一个任务置于睡眠状态，并将其从运行队列中移除。
- `wake_up(&mut self, task: SharedTask)`: 唤醒一个睡眠中的任务，将其重新放回运行队列。
- `exit_task(&mut self, task: SharedTask, ...)`: 处理一个任务的退出，将其从调度系统中永久移除。

## 2. 轮转调度器 (`RRScheduler`)

当前内核中实现的具体调度策略是简单的 **轮转调度（Round-Robin Scheduler）**。

**源码链接**: [`os/src/kernel/scheduler/rr_scheduler.rs`](/os/src/kernel/scheduler/rr_scheduler.rs)

### 实现机制

- **运行队列**: `RRScheduler` 内部使用一个 `TaskQueue`（基于 `Vec` 的 FIFO 队列）作为运行队列。新加入的任务被放在队尾，调度器总是从队首取出任务执行。
- **时间片**: 每个任务被分配一个固定的时间片（`DEFAULT_TIME_SLICE`）。当时钟中断发生时，`RRScheduler::update_time_slice` 方法会被调用，减少当前任务的剩余时间片。当时间片耗尽时，就会触发一次抢占式调度。

## 3. 调度时机

调度器在以下几个关键时刻被触发，以决定是否切换任务：

1.  **时钟中断（抢占式调度）**:
    - `riscv::timer` 模块设置了定时器，在固定间隔后触发时钟中断。
    - 中断处理程序 `trap_handler` 会调用 `schedule()`。
    - `schedule()` 内部会检查当前任务的时间片是否耗尽。如果是，则会执行 `prepare_switch` 来选择下一个任务，实现抢占。

2.  **任务主动让出 (`yield`)**:
    - 任务可以调用 `yield_task()` 主动放弃 CPU。
    - `yield_task()` 会直接调用 `schedule()`，立即触发一次调度，将当前任务放回运行队列末尾，并切换到下一个任务。

3.  **任务阻塞**:
    - 当任务因等待资源（如 `SleepLock`）而需要睡眠时，它会调用 `sleep_task()`。
    - `sleep_task()` 将任务从运行队列中移除，并改变其状态为 `Interruptible` 或 `Uninterruptible`。
    - 随后会调用 `schedule()` 来切换到一个新的可运行任务。

## 4. 上下文切换流程

上下文切换是调度机制的核心，它实现了 CPU 执行流从一个任务到另一个任务的平滑过渡。

**核心函数**: `schedule()` in [`os/src/kernel/scheduler/mod.rs`](/os/src/kernel/scheduler/mod.rs)

**流程详解**:

1.  **触发调度**: 当上述任一调度时机发生时，`schedule()` 函数被调用。

2.  **准备切换 (`prepare_switch`)**:
    - `schedule()` 函数会调用当前调度器（`RRScheduler`）的 `prepare_switch` 方法。
    - `prepare_switch` 从 CPU 的本地存储中取出当前任务 (`prev_task`)，并从运行队列中选出下一个任务 (`next_task`)。
    - 如果没有其他可运行任务，则不进行切换。
    - 如果有，它会获取 `prev_task` 和 `next_task` 的上下文指针 (`Context`)。
    - 如果 `prev_task` 仍然是可运行状态（例如，时间片用完但未阻塞），它会被重新放回运行队列的末尾。
    - 最后，更新 CPU 的当前任务为 `next_task`，并返回包含新旧上下文指针的 `SwitchPlan`。

3.  **执行切换 (`__switch`)**:
    - `schedule()` 函数拿到 `SwitchPlan` 后，会调用一个底层的汇编函数 `__switch(old_ctx_ptr, new_ctx_ptr)`。
    - **源码链接**: [`os/src/arch/riscv/kernel/switch.S`](/os/src/arch/riscv/kernel/switch.S)
    - `__switch` 函数执行以下操作：
        a. **保存旧上下文**: 将当前任务（`old_task`）的 callee-saved 寄存器（如 `ra`, `sp`, `s0-s11`）保存到其 `Context` 结构体中（由 `old_ctx_ptr` 指向）。
        b. **恢复新上下文**: 从新任务（`next_task`）的 `Context` 结构体中（由 `new_ctx_ptr` 指向），将之前保存的寄存器值加载回 CPU 的物理寄存器。
        c. **返回**: `__switch` 函数的最后一条指令是 `ret`。它会跳转到新任务 `Context` 中保存的 `ra` (返回地址)。
            - 对于一个从未执行过的新任务，其 `ra` 在创建时被设置为 `forkret`。
            - 对于一个之前被切换出去的任务，其 `ra` 指向它被切换时 `__switch` 调用的下一条指令。

通过这个流程，CPU 的执行状态被完整地从一个任务切换到另一个任务，实现了多任务的并发执行。