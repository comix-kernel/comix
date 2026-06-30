# RwLock

`RwLock<T>` 允许多个读者并发访问或一个写者独占访问.它适合读多写少且临界区仍然很短的内核数据.

## 当前状态

- 状态由一个 `AtomicUsize` 编码.
- 高位 `WRITER_BIT` 表示写者持有.
- 低位 `READER_MASK` 表示读者数量.
- 读写 guard 都持有 `IntrGuard`, 因此 guard 生命周期内本 CPU 中断被禁用.

## 目标

- 降低读多写少场景的无谓互斥.
- 维持与自旋锁相似的中断安全约束.
- 用 RAII guard 管理读者计数和写者位.

## 非目标

- 不提供写者优先或公平调度.
- 不支持读锁升级为写锁, 也不支持写锁降级为读锁.
- 不适合长临界区.

## 关键流程

### 读锁

1. 创建 `IntrGuard`.
2. 读取状态.如果写者位存在, 自旋等待.
3. CAS 把读者计数加一.
4. 读 guard drop 时 release 减一.

### 写锁

1. 创建 `IntrGuard`.
2. 只有状态为 0 时 CAS 设置写者位.
3. 写 guard 提供可变访问.
4. 写 guard drop 时 release 清零状态.

## 并发约束

- 读者和写者都在持 guard 期间禁用本 CPU 中断.
- 写者可能被源源不断的新读者延迟, 当前实现没有饥饿防护.
- 读者计数达到 mask 上限会 panic, 这是不应出现的内核错误.
- 临界区不能主动 sleep 或等待调度.

## 已知限制

- 无公平性.
- 无锁升级/降级.
- 状态编码固定使用 `usize` 的 bit layout, 文档不承诺外部依赖这些常量.

## 源码索引

- `os/src/sync/rwlock.rs:12` - 状态位定义.
- `os/src/sync/rwlock.rs:24` - `RwLock<T>`.
- `os/src/sync/rwlock.rs:51` - `read()`.
- `os/src/sync/rwlock.rs:79` - `write()`.
- `os/src/sync/rwlock.rs:98` - `try_read()`.
- `os/src/sync/rwlock.rs:125` - `try_write()`.
- `os/src/sync/rwlock.rs:153` - guard 访问和 drop.
