# 任务结构及生命周期管理

本文档详细说明了 `Task` 的核心数据结构、生命周期状态转换以及暴露给其他内核模块的接口。

## 1. 核心数据结构：`Task`

操作系统的核心任务表示是 `Task` 结构体，它统一了进程和线程的概念。所有与执行流相关的信息都封装在其中。

**源码链接**: [`os/src/kernel/task/task_struct.rs`](/os/src/kernel/task/task_struct.rs)

`Task` 结构体的主要字段可以分为以下几类：

### 1.1 身份与亲属关系

这些字段用于唯一标识一个任务并建立任务间的层级关系。

- `tid: u32`: **任务ID (Task ID)**。由 `TidAllocator` 分配的全系统唯一标识符。
- `pid: u32`: **进程ID (Process ID)**。对于线程，它与创建它的主任务 `pid` 相同。对于一个进程的第一个任务，`pid` 等于其 `tid`。
- `ppid: u32`: **父任务ID (Parent Process ID)**。

### 1.2 调度与执行上下文

这些字段由调度器和中断处理机制在任务切换和执行时使用。

- `context: Context`: **任务上下文**。保存了任务切换时需要恢复的最小寄存器集合（主要是 `ra` 和 `sp`），用于非中断驱动的上下文切换。
- `trap_frame_ptr: AtomicPtr<TrapFrame>`: **中断帧指针**。当任务从用户态或内核态陷入(trap)时，CPU的完整上下文（所有通用寄存器、`sepc`、`sstatus`等）被保存在其内核栈上，此指针指向该 `TrapFrame` 的位置。当中断返回时，`__restore` 会用它来恢复现场。
- `state: TaskState`: **任务状态**。定义在 [`os/src/kernel/task/task_state.rs`](/os/src/kernel/task/task_state.rs)，是任务生命周期管理的核心。
- `preempt_count: usize`: **抢占计数器**。当大于0时，禁止内核抢占，用于保护临界区。

### 1.3 资源管理

这些字段管理任务执行所必需的系统资源。

- `kstack_base: usize`: **内核栈顶地址**。每个任务都有自己独立的内核栈。
- `kstack_tracker` & `trap_frame_tracker`: 用于跟踪内核栈和中断帧所占用的物理页帧，以便在任务销毁时正确回收。
- `memory_space: Option<Arc<MemorySpace>>`: **内存地址空间**。对于用户任务，它包含了页表、内存映射区域等信息。对于内核线程，此字段为 `None`。

### 1.4 生命周期与退出状态

这些字段用于处理任务的终止和父任务的等待。

- `exit_code: Option<i32>`: **退出码**。用于进程，在调用 `exit` 系统调用时设置。
- `return_value: Option<usize>`: **返回值**。用于线程，在线程函数返回时设置。

## 2. 任务的生命周期与状态转换

任务的生命周期由 `TaskState` 枚举驱动，并通过一系列接口函数进行管理。

### 2.1 任务的创建

#### 内核线程

- **接口**: `kthread_spawn(name: &'static str, entry: fn(usize) -> !, arg: usize)`
- **源码**: [`os/src/kernel/task/ktask.rs`](/os/src/kernel/task/ktask.rs)
- **机制**:
    1. 调用 `TASK_MANAGER` 分配 `tid`。
    2. 分配内核栈和中断帧所需的物理内存。
    3. 调用 `Task::ktask_create` 创建 `Task` 实例。此函数会：
        - 初始化 `Context`，将 `ra` 指向 `forkret`，`sp` 指向内核栈顶。
        - 初始化位于内核栈上的 `TrapFrame`，将 `sepc` 设置为线程入口点 `entry`，`sstatus` 设置为S模式，并设置好内核栈指针 `x2_sp`。
    4. 将新创建的任务包装在 `Arc<SpinLock<Task>>` (即 `SharedTask`) 中，并交给调度器 `SCHEDULER` 的运行队列。

#### 用户任务 (待实现)

- **接口**: (例如 `utask_create` 或 `sys_clone`)
- **机制**: 与内核线程类似，但需要额外创建和关联一个 `MemorySpace`（用户地址空间），并初始化 `TrapFrame` 以便从S模式返回到U模式执行。

### 2.2 任务的执行与切换

- **`forkret`**:
    - **源码**: [`os/src/kernel/task/mod.rs`](/os/src/kernel/task/mod.rs)
    - **机制**: 所有新创建的任务在第一次被调度器选中时，都会从 `__switch` 跳转到 `forkret` 函数。`forkret` 的唯一职责是从当前任务的 `trap_frame_ptr` 中加载中断帧地址，并调用 `restore` 汇编例程。`restore` 会将中断帧中的寄存器值恢复到CPU中，最后通过 `sret` 指令跳转到任务的真正入口点（`sepc`），任务从而开始执行。

### 2.3 任务的睡眠与唤醒

- **接口**:
    - `sleep_task(task: SharedTask, receive_signal: bool)`
    - `wake_up(task: SharedTask)`
- **源码**: [`os/src/kernel/scheduler/rr_scheduler.rs`](/os/src/kernel/scheduler/rr_scheduler.rs) (作为 `Scheduler` trait 的一部分)
- **机制**:
    - **睡眠**: 当任务需要等待资源时（例如等待一个 `SleepLock`），它会调用 `sleep_task`。调度器会将该任务的 `state` 设置为 `Interruptible` 或 `Uninterruptible`，并将其从运行队列中移除。随后调度器会选择下一个任务运行。
    - **唤醒**: 当资源可用时，持有该资源的模块会调用 `wake_up`。调度器会将任务的 `state` 恢复为 `Running`，并将其重新加入运行队列。

### 2.4 任务的终止

- **接口**: `terminate_task(return_value: usize) -> !`
- **源码**: [`os/src/kernel/task/mod.rs`](/os/src/kernel/task/mod.rs)
- **机制**:
    1. 当一个内核线程的入口函数返回时，`TrapFrame` 中预设的返回地址 `ra` 会指向 `terminate_task`。
    2. `terminate_task` 获取当前任务，将其 `state` 设置为 `Stopped`，并保存返回值 `return_value`。
    3. 它主动调用 `schedule()` 让出CPU，由于任务状态已是 `Stopped`，它将不会再被调度器放回运行队列。
    4. 任务占用的资源（如内核栈）的最终回收依赖于 `Arc` 的引用计数。当所有对该任务的 `SharedTask` 引用都消失后，`Task` 的 `Drop` 实现会被调用，从而释放内存。

## 3. 暴露接口

- **`os/src/kernel/task/mod.rs`**:
    - `SharedTask`: `Arc<SpinLock<TaskStruct>>` 的类型别名，是任务在内核中传递的标准形式。
    - `into_shared(task: TaskStruct) -> SharedTask`: 将一个 `Task` 结构包装为 `SharedTask`。
- **`os/src/kernel/task/ktask.rs`**:
    - `kthread_spawn(...)`: 创建内核线程的顶层API。
- **`os/src/kernel/mod.rs` (通过 `scheduler` 模块暴露)**:
    - `yield_task()`: 主动让出CPU，触发一次调度。
    - `sleep_task(...)`: 使指定任务进入睡眠状态。
    - `wake_up(...)`: 唤醒指定任务。
    - `exit_task(...)`: 终止指定任务并设置退出码。

这些接口共同构成了任务管理的核心功能，为上层模块（如锁、IPC、系统调用）提供了构建并发服务的基础。