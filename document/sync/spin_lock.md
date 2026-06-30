# SpinLock

`SpinLock<T>` 是带数据的短临界区互斥锁.它把 `RawSpinLock` 和 `UnsafeCell<T>` 组合起来, 通过 guard 提供 `Deref`/`DerefMut` 访问.

## 当前状态

- 内部锁是 `RawSpinLock`.
- 成功加锁后返回 `SpinLockGuard`.
- guard 生命周期内可访问被保护数据.
- `try_lock()` 在竞争时返回 `None`.

## 目标

- 保护极短的共享数据修改.
- 同时处理跨 CPU 竞争和本 CPU 中断重入.
- 作为全局状态和内核基础设施的简单锁.

## 非目标

- 不提供公平性.
- 不适合可能 sleep, yield 或长时间等待的代码.
- 不可重入.

## 关键流程

1. `SpinLock::lock()` 进入 `RawSpinLock::lock()`.
2. `RawSpinLock` 禁用本 CPU 中断并自旋获取原子锁.
3. `SpinLockGuard` 保存 raw guard 和 `&mut T`.
4. guard drop 时 raw guard 释放锁并恢复中断状态.

## 并发约束

- 持锁代码必须短小.
- 持锁期间不要调用可能调度的路径, 包括会等待任务队列的 `Mutex<T>`.
- 在中断上下文中只能使用不会睡眠的路径.
- 多把锁嵌套时必须遵守全局锁顺序, 避免死锁.

## 已知限制

- 高竞争下会消耗 CPU 周期.
- 没有所有者记录, 不能检测当前 CPU 是否重复加锁.

## 源码索引

- `os/src/sync/spin_lock.rs:30` - `SpinLock<T>`.
- `os/src/sync/spin_lock.rs:53` - `lock()`.
- `os/src/sync/spin_lock.rs:62` - `try_lock()`.
- `os/src/sync/spin_lock.rs:77` - `SpinLockGuard`.
