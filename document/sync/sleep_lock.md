# SleepLock 状态

当前代码库没有独立的 sleep lock 模块, `sync` 模块也没有导出 `SleepLock`. 本页保留用于说明历史文档状态, 不应被当作现行实现说明.

## 当前替代

需要"竞争时让出 CPU"的任务上下文互斥, 请查看 [Mutex](mutex.md). 当前 `Mutex<T>` 使用 `AtomicBool`, `RawSpinLock` 和 `WaitQueue` 组合实现, 并通过 `MutexGuard` 管理解锁和唤醒.

## 下线原因

旧文档描述了一个独立无数据 `SleepLock`, 但当前源码中的正式类型是带数据的 `Mutex<T>`:

- `os/src/sync/mod.rs` 导出 `mutex::*`, 没有 `sleep_lock` 模块.
- `os/src/sync/mutex.rs` 是当前睡眠式互斥实现.
- `SpinLock` 和 `RawSpinLock` 文档中的"长临界区用 SleepLock"说法已改为指向 `Mutex<T>`.

## 对使用者的影响

- 不要引用 `crate::sync::SleepLock`.
- 不要链接历史 sleep lock 模块路径.
- 文档导航后续应由主 agent 决定是否移除本页或移动到历史区.

## 源码索引

- `os/src/sync/mod.rs:5` - 当前同步模块列表.
- `os/src/sync/mutex.rs:21` - 当前睡眠式互斥 `Mutex<T>`.
