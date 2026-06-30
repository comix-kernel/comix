# 同步与锁

`os/src/sync` 提供 Comix 内核当前使用的同步原语. 本文档只描述现行实现和设计边界, 不把未接入源码的历史方案当作可用 API.

## 当前状态

当前导出的核心原语:

- `RawSpinLock`
- `SpinLock<T>`
- `RwLock<T>`
- `Mutex<T>`
- `IntrGuard`
- `PreemptGuard`
- `PerCpu<T>`

当前 `os/src/sync` 中没有 `ticket_lock.rs` 或 `sleep_lock.rs`. 对应文档页仅用于说明历史或未实现状态, 不进入主导航.

## 设计目标

- 用 RAII guard 绑定锁生命周期, 避免遗漏释放.
- 在短临界区内同时处理跨 CPU 竞争和本 CPU 中断重入.
- 为较长临界区提供会让出 CPU 的 `Mutex<T>`.
- 为 per-CPU 数据提供抢占保护约束.
- 让底层锁可以作为 `lock_api::RawMutex` 服务第三方组件, 例如 talc allocator.

## 非目标

- 不提供严格 FIFO 公平锁.
- 不提供独立的无数据 `SleepLock`.
- 不提供用户态同步 ABI.
- 不保证所有锁可在中断上下文使用. 会调度或睡眠的路径不能在中断上下文使用.

## 原语分层

```text
IntrGuard
  - 禁用和恢复本 CPU 中断
  - 支持 per-CPU 嵌套深度

RawSpinLock
  - AtomicBool 互斥
  - 进入时持有 IntrGuard
  - 也实现 lock_api::RawMutex

SpinLock<T>
  - RawSpinLock + UnsafeCell<T>
  - 短临界区互斥

RwLock<T>
  - AtomicUsize 状态
  - 多读或单写
  - 持 guard 期间禁用本 CPU 中断

Mutex<T>
  - AtomicBool + RawSpinLock + WaitQueue
  - 竞争时入队并 yield

PreemptGuard + PerCpu<T>
  - 防止访问当前 CPU 数据时任务迁移
```

## 选择建议

| 场景 | 当前原语 |
| --- | --- |
| 极短共享数据修改 | `SpinLock<T>` |
| 读多写少且临界区短 | `RwLock<T>` |
| allocator 或底层锁适配 | `RawSpinLock` |
| 可能等待较久的任务上下文互斥 | `Mutex<T>` |
| 单 CPU 中断重入屏蔽 | `IntrGuard` |
| 访问当前 CPU 本地数据 | `PreemptGuard` + `PerCpu<T>` |

## 并发约束

- `SpinLock`, `RawSpinLock`, `RwLock` 都会屏蔽本 CPU 中断, 但仍依赖原子操作处理跨 CPU 竞争.
- 自旋锁类临界区必须短, 不能主动 sleep 或长期等待调度.
- `Mutex<T>` 可能调用调度相关路径, 只适合任务上下文.
- `PerCpu<T>::get_mut()` 从共享引用返回当前 CPU 的可变引用, 调用方必须用 `PreemptGuard` 或等价机制保证期间不会迁移.

## 文档导航

当前设计页:

- [RawSpinLock](raw_spin_lock.md)
- [SpinLock](spin_lock.md)
- [RwLock](rwlock.md)
- [Mutex](mutex.md)
- [IntrGuard](intr_guard.md)
- [PreemptGuard](preempt.md)
- [PerCpu](per_cpu.md)
- [死锁预防](deadlock.md)
- [SMP 中断与并发](smp_interrupts.md)

历史状态页:

- [TicketLock 状态](ticket_lock.md)
- [SleepLock 状态](sleep_lock.md)

## 源码索引

- `os/src/sync/mod.rs:5` - 当前模块列表和导出集合.
- `os/src/sync/raw_spin_lock.rs:20` - `RawSpinLock`.
- `os/src/sync/spin_lock.rs:30` - `SpinLock<T>`.
- `os/src/sync/rwlock.rs:24` - `RwLock<T>`.
- `os/src/sync/mutex.rs:21` - `Mutex<T>`.
- `os/src/sync/intr_guard.rs:44` - `IntrGuard`.
- `os/src/sync/preempt.rs:95` - `PreemptGuard`.
- `os/src/sync/per_cpu.rs:36` - `PerCpu<T>`.
