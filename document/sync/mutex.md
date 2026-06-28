# Mutex

`Mutex<T>` 是当前同步模块中的睡眠式互斥原语.和 `SpinLock<T>` 不同, 它在竞争时会把当前任务加入等待队列并让出 CPU.

## 当前状态

- 源码文件是 `os/src/sync/mutex.rs`.
- 状态由 `AtomicBool locked` 表示.
- 内部 `RawSpinLock` 保护检查和入队过程.
- 等待者保存在 `SpinLock<WaitQueue>` 中.
- 成功持锁时返回 `MutexGuard`, guard drop 时清除 `locked` 并唤醒等待者.

## 目标

- 为任务上下文中的较长临界区提供互斥.
- 竞争时避免长时间忙等.
- 用 guard 绑定解锁和唤醒.

## 非目标

- 不适合中断上下文.
- 不提供严格 FIFO 唤醒语义.
- 不替代所有短临界区自旋锁.热路径仍应优先考虑 `SpinLock<T>` 或更细粒度设计.

## 关键流程

1. 获取内部 `RawSpinLock`.
2. 如果 `locked` 原来为 false, 设置为 true 并返回 `MutexGuard`.
3. 如果锁已占用, 获取当前任务, 把任务放入等待队列.
4. 释放内部 raw lock, 调用 `yield_task()` 让出 CPU.
5. 被唤醒后重新尝试.
6. guard drop 时把 `locked` 置 false 并唤醒等待队列.

## 并发与生命周期约束

- `Mutex<T>` 依赖调度器和 `WaitQueue`, 只能在有当前任务的上下文使用.
- 不要在持有 `SpinLock` 或禁用中断的长路径中等待 `Mutex<T>`.
- guard 当前保存了内部 raw lock guard, 因此持有 mutex 数据期间也会保持 raw lock 的中断保护语义.调用方仍应避免长时间关中断的工作.
- 唤醒使用 `wake_up_all()`, 被唤醒任务会重新竞争 `locked`.

## 已知限制

- 当前实现更接近初版睡眠互斥, 等待队列公平性和惊群控制仍可改进.
- 没有 `try_lock()`.
- 没有所有者检查或死锁诊断.

## 源码索引

- `os/src/sync/mutex.rs:21` - `Mutex<T>` 字段.
- `os/src/sync/mutex.rs:31` - 构造.
- `os/src/sync/mutex.rs:41` - 竞争和等待流程.
- `os/src/sync/mutex.rs:60` - `MutexGuard`.
- `os/src/sync/mutex.rs:79` - guard drop 解锁和唤醒.
