# 睡眠锁 (`SleepLock`)

`SleepLock` 是一种互斥锁，当任务尝试获取一个已被占用的锁时，它不会自旋空等，而是会将任务置于**睡眠状态**，并让出CPU给其他任务执行。

**源码链接**: [`os/src/sync/sleep_lock.rs`](/os/src/sync/sleep_lock.rs)

## 1. 工作原理

`SleepLock` 的实现依赖于调度器和 `WaitQueue` 机制。

1.  **内部状态**: `SleepLock` 内部包含一个布尔值 `locked` 表示锁状态，一个 `RawSpinLock` 用于保护 `locked` 字段本身，以及一个 `WaitQueue` 用于管理等待此锁的睡眠任务。

2.  **获取锁 (`lock`)**:
    a. 任务尝试获取锁。它首先获取内部的 `RawSpinLock`。
    b. 检查 `locked` 字段。如果锁未被占用 (`false`)，则将 `locked` 设置为 `true`，释放 `RawSpinLock`，获取锁成功。
    c. 如果锁已被占用 (`true`)，任务会将自己加入到 `WaitQueue` 中，然后调用 `sleep_task()` 进入睡眠状态。在睡眠前，它会释放内部的 `RawSpinLock`。

3.  **释放锁 (`unlock`)**:
    a. 持有锁的任务完成操作后调用 `unlock()`。
    b. 它获取内部的 `RawSpinLock`，将 `locked` 设置为 `false`。
    c. 调用 `WaitQueue` 的 `wake_up_one()` 方法，唤醒一个正在等待队列中睡眠的任务。
    d. 释放 `RawSpinLock`。被唤醒的任务将有机会在下一次调度时运行，并再次尝试获取锁。

## 2. 核心接口

- `pub fn new() -> Self`: 创建一个新的 `SleepLock`。注意它不直接包裹数据，通常用于保护一段代码逻辑。
- `pub fn lock(&mut self)`: 获取锁。如果锁被占用，将阻塞当前任务（使其睡眠）。
- `pub fn unlock(&mut self)`: 释放锁，并唤醒一个等待者。

## 3. 适用场景

**优点**:
- 当锁的争用激烈或临界区执行时间较长时，它不会像自旋锁那样浪费CPU资源，而是通过任务调度提高了系统整体的吞吐量。

**缺点**:
- 涉及任务的睡眠和唤醒，有上下文切换的开销，因此比 `SpinLock` 更“重”。
- **`SleepLock` 只能在任务上下文中使用，绝对不能在中断处理程序中使用**，因为中断处理程序没有任务上下文，无法被调度或睡眠。

**结论**: `SleepLock` **适用于保护那些访问时间较长或可能发生阻塞的临界区**。例如，文件系统操作、复杂的设备I/O等。
