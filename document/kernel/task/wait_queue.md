# 等待队列 (`WaitQueue`)

等待队列是实现任务同步和阻塞的核心机制。当一个任务需要等待某个条件（如锁被释放、I/O完成）才能继续执行时，它就会被放入一个等待队列中并进入睡眠状态。

**源码链接**: [`os/src/kernel/scheduler/wait_queue.rs`](/os/src/kernel/scheduler/wait_queue.rs)

## 1. 设计与目的

`WaitQueue` 的主要职责是管理一组正在等待同一个事件的睡眠任务。它通常与一个自旋锁 (`RawSpinLock`) 配合使用，以保证在多核环境下的操作原子性。

其核心数据结构如下：
```rust
// os/src/kernel/scheduler/wait_queue.rs
pub struct WaitQueue {
    tasks: TaskQueue,
    lock: RawSpinLock,
}
```
- `tasks`: 一个 `TaskQueue`，用于存放等待此队列的 `SharedTask` 句柄。
- `lock`: 一个自旋锁，用于保护 `tasks` 队列在并发访问时的数据一致性。

## 2. 核心接口与工作流程

### `sleep(task: SharedTask)`

当一个任务需要阻塞等待时，持有资源的模块（如 `SleepLock`）会调用此方法。

1.  获取 `WaitQueue` 的内部锁 `lock`。
2.  将需要睡眠的 `task` 添加到内部的 `tasks` 队列中。
3.  释放 `lock` 锁。
4.  调用调度器提供的 `sleep_task(task, ...)` 函数，将任务状态设置为 `Interruptible` 或 `Uninterruptible`，并将其从调度器的运行队列中移除。
5.  触发一次调度 (`schedule()`)，CPU切换到其他可运行的任务。

**关键点**: 必须在调用 `sleep_task` **之前** 释放 `WaitQueue` 的内部锁，以避免在持有锁的情况下进行任务调度，这可能导致死锁。

### `wake_up_one()`

当等待的条件满足时，持有资源的模块会调用此方法来唤醒一个等待的任务。

1.  获取 `WaitQueue` 的内部锁 `lock`。
2.  从 `tasks` 队列的队首弹出一个任务（`pop_task`）。
3.  释放 `lock` 锁。
4.  如果成功弹出了一个任务，则调用调度器提供的 `wake_up(task)` 函数。
5.  `wake_up` 函数会将任务的状态改回 `Running`，并将其重新加入到调度器的运行队列中，使其有机会在下一次调度时被执行。

### `wake_up_all()`

此方法用于唤醒等待队列中的所有任务，流程与 `wake_up_one` 类似，但它会遍历并清空整个 `tasks` 队列，并逐个唤醒所有任务。

## 3. 应用示例：`SleepLock`

`SleepLock` 是 `WaitQueue` 的一个典型应用场景。

**源码链接**: [`os/src/sync/sleep_lock.rs`](/os/src/sync/sleep_lock.rs)

- **`lock()`**:
    1. 尝试获取锁。如果锁已被占用，则获取当前任务的句柄。
    2. 调用 `SleepLock` 内部 `WaitQueue` 的 `sleep()` 方法，将当前任务放入等待队列并使其睡眠。
    3. 当任务被唤醒后，它会回到 `lock()` 的循环开头，再次尝试获取锁。

- **`unlock()`**:
    1. 释放锁。
    2. 调用 `WaitQueue` 的 `wake_up_one()` 方法，唤醒一个正在等待此锁的任务。

通过 `WaitQueue`，`SleepLock` 实现了当锁不可用时，任务会放弃CPU进入睡眠，而不是空耗CPU进行自旋等待，从而大大提高了系统效率。
