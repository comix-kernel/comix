# TicketLock 状态

当前代码库没有独立的 ticket lock 模块, `sync` 模块也没有导出 `TicketLock`. 本页保留用于说明历史或未实现状态, 不应被当作现行实现说明.

## 当前替代

需要短临界区互斥时, 使用 [SpinLock](spin_lock.md) 或 [RawSpinLock](raw_spin_lock.md). 需要读多写少时, 使用 [RwLock](rwlock.md).

当前这些锁不提供严格 FIFO 公平性:

- `RawSpinLock` 使用 `AtomicBool` CAS 自旋.
- `SpinLock<T>` 只是 `RawSpinLock` 加数据.
- `RwLock<T>` 使用读者计数和写者位, 没有写者优先或公平队列.

## 未实现内容

旧文档曾描述基于 `next_ticket` 和 `serving_ticket` 的 FIFO 自旋锁, 但当前源码没有对应类型, guard 或测试. 任何需要公平锁的设计都应先补实现和测试, 再恢复正式文档.

## 对使用者的影响

- 不要引用 `crate::sync::TicketLock`.
- 不要链接历史 ticket lock 模块路径.
- 不要在设计文档中把 FIFO 公平性列为当前同步模块能力.
- 文档导航后续应由主 agent 决定是否移除本页或移动到历史区.

## 源码索引

- `os/src/sync/mod.rs:5` - 当前同步模块列表.
- `os/src/sync/raw_spin_lock.rs:20` - 当前底层自旋锁.
- `os/src/sync/spin_lock.rs:30` - 当前带数据自旋锁.
- `os/src/sync/rwlock.rs:24` - 当前读写锁.
