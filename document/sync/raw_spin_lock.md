# RawSpinLock

`RawSpinLock` 是同步模块的底层互斥构件.它只管理锁状态和中断保护, 不直接暴露受保护数据.

## 当前状态

- 锁状态是 `AtomicBool`.
- 普通 guard 路径在加锁前创建 `IntrGuard`, 加锁成功后由 `RawSpinLockGuard` 保存该 guard.
- `try_lock()` 获取失败时会让临时 `IntrGuard` drop, 立即恢复进入前中断状态.
- 同时实现 `lock_api::RawMutex`, 供 talc 等外部抽象使用.

## 目标

- 作为 `SpinLock<T>` 的底层互斥.
- 为全局 allocator 提供 `lock_api` 兼容锁.
- 在本 CPU 中断上下文可能重入相同数据时, 通过禁用中断降低死锁风险.

## 非目标

- 不保护具体数据, 上层必须自己组合 `UnsafeCell` 或其他容器.
- 不提供公平性保证.
- 不可重入.持有后再次获取同一把锁会自旋等待自己释放.

## 关键流程

### 普通 guard 路径

1. 创建 `IntrGuard`, 禁用本 CPU 中断.
2. 用 acquire CAS 把锁状态从 false 改成 true.
3. 返回 `RawSpinLockGuard`.
4. guard drop 时先 release 存储 false, 然后字段 drop 恢复中断状态.

### lock_api 路径

`lock_api::RawMutex` 接口没有显式 guard 字段保存 `IntrGuard`, 因此实现会保存进入锁前的中断 flags.unlock 时释放锁并恢复该 flags.

## 并发约束

- 临界区必须短.
- 不能在持锁期间主动调度或执行可能长期等待的操作.
- `RawSpinLock` 可跨 CPU 共享, 互斥靠原子 acquire/release 语义.
- 中断状态只针对当前 CPU, 不能替代跨 CPU 原子互斥.

## 已知限制

- `lock_api` 适配路径用锁内单个 `saved_intr_flags` 保存 flags, 设计上要求同一把 raw lock 不可重入, 且 unlock 与成功 lock 配对.
- 没有队列或退避策略, 高竞争时会持续自旋.

## 源码索引

- `os/src/sync/raw_spin_lock.rs:20` - 锁状态.
- `os/src/sync/raw_spin_lock.rs:61` - acquire 自旋循环.
- `os/src/sync/raw_spin_lock.rs:80` - 普通 `lock()`.
- `os/src/sync/raw_spin_lock.rs:93` - `try_lock()`.
- `os/src/sync/raw_spin_lock.rs:118` - `RawSpinLockGuard`.
- `os/src/sync/raw_spin_lock.rs:132` - `lock_api::RawMutex` 实现.
