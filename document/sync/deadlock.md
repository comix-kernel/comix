# 锁顺序与死锁预防

死锁预防依赖一致的锁顺序和对上下文的限制.当前同步原语中, `SpinLock`, `RawSpinLock`, `RwLock` 属于自旋类短临界区, `Mutex<T>` 属于任务上下文等待类互斥.

## 当前规则

1. 不要在持有自旋类锁时等待会调度的资源.
2. 不要在中断上下文获取 `Mutex<T>`.
3. 多把锁嵌套时保持全局一致顺序.
4. 持有 `IntrGuard` 或自旋类 guard 的代码必须短.
5. 访问 `PerCpu<T>` 当前 CPU 可变副本时持有 `PreemptGuard`; 如果中断也访问同一数据, 额外处理中断重入.

## 推荐锁顺序

从低层到高层:

1. CPU 本地状态 guard: `PreemptGuard`, `IntrGuard`.
2. 底层自旋锁: `RawSpinLock`.
3. 数据自旋锁: `SpinLock<T>`, `RwLock<T>`.
4. 等待队列和任务上下文互斥: `Mutex<T>` 及其内部 `WaitQueue`.
5. 文件系统, 进程, 设备等更高层对象锁.

实际代码如果需要更细锁序, 应在所属子系统文档中声明.

## 常见风险

### 自旋锁内等待 Mutex

`Mutex<T>` 竞争时会把任务入队并 `yield_task()`.如果当前已经持有自旋锁或关闭中断, 调度和唤醒路径可能无法前进.

### 中断上下文等待任务资源

中断处理程序没有普通任务睡眠语义, 只能使用不会 sleep 的同步路径, 并且临界区必须短.

### Per-CPU 数据迁移

没有 `PreemptGuard` 时, 任务可能先根据 CPU A 的 ID 取到引用, 随后迁移到 CPU B 继续访问, 破坏 per-CPU 语义.

### RwLock 升级

当前 `RwLock` 不支持读锁升级写锁.持读锁时再尝试写锁可能等待自己释放.

## 已知限制

- 当前锁实现没有统一的 owner 追踪和死锁检测.
- `Mutex<T>` 唤醒等待者后仍由任务重新竞争, 不提供严格公平顺序.
- `TicketLock` 不是当前实现能力, 不能依赖 FIFO 锁顺序解决饥饿.

## 源码索引

- `os/src/sync/raw_spin_lock.rs:80` - 自旋类底层加锁.
- `os/src/sync/spin_lock.rs:53` - 带数据自旋锁.
- `os/src/sync/rwlock.rs:51` - 读锁获取.
- `os/src/sync/rwlock.rs:79` - 写锁获取.
- `os/src/sync/mutex.rs:41` - 等待式互斥获取.
- `os/src/sync/preempt.rs:95` - 抢占 guard.
- `os/src/sync/intr_guard.rs:44` - 中断 guard.
