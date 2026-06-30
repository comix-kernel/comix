# SMP 中断与并发

SMP 内核中的并发来源不止任务之间的抢占, 还包括本 CPU 中断重入和其他 CPU 同时执行.同步原语必须明确自己覆盖哪一种并发.

## 并发来源

- 同一 CPU 上任务被中断处理程序打断.
- 多个 CPU 同时运行任务代码.
- 多个 CPU 同时处理共享中断或 IPI.
- 调度迁移导致任务访问不同 CPU 的 per-CPU 数据.

## 当前原语覆盖范围

| 原语 | 覆盖本 CPU 中断重入 | 覆盖跨 CPU 竞争 | 可能调度 |
| --- | --- | --- | --- |
| `IntrGuard` | 是 | 否 | 否 |
| `RawSpinLock` | 是 | 是 | 否 |
| `SpinLock<T>` | 是 | 是 | 否 |
| `RwLock<T>` | 是 | 是 | 否 |
| `Mutex<T>` | 否, 不适合中断上下文 | 是 | 是 |
| `PreemptGuard` | 否 | 否 | 否 |
| `PerCpu<T>` | 否 | 通过分片减少共享 | 否 |

## 关键规则

### IntrGuard 不是 SMP 锁

禁用本 CPU 中断不能阻止其他 CPU 访问同一全局变量.跨 CPU 共享数据需要 `RawSpinLock`, `SpinLock<T>`, `RwLock<T>` 或更高层同步.

### 自旋类锁必须短

自旋锁持有期间本 CPU 中断被关闭.临界区越长, 中断延迟越大, 其他 CPU 自旋浪费也越多.

### Mutex 只能在任务上下文

`Mutex<T>` 竞争时会使用 `WaitQueue`, `current_task()` 和 `yield_task()`.中断上下文不能依赖这些语义.

### Per-CPU 数据需要防迁移

`PerCpu<T>` 把共享数据拆成每 CPU 副本, 但访问当前 CPU 副本期间必须避免迁移.通常使用 `PreemptGuard`.

## TLB 与 IPI

MM 页表后端也受 SMP 约束:

- RISC-V 修改页表后会刷新本地 TLB, 多核时通过 IPI 通知其他 CPU.
- 批处理上下文用于减少多页操作中的 IPI 次数.
- LoongArch64 当前批处理上下文只保证本地刷新, 跨核 shootdown 仍是限制.

## 已知限制

- 同步模块没有统一的中断上下文检查.
- 没有跨 CPU 锁依赖图或运行时死锁检测.
- `Mutex<T>` 等待队列公平性仍是基础实现.

## 源码索引

- `os/src/sync/intr_guard.rs:44` - 本 CPU 中断保护.
- `os/src/sync/raw_spin_lock.rs:20` - 中断保护加原子互斥.
- `os/src/sync/mutex.rs:41` - 任务上下文等待式互斥.
- `os/src/sync/preempt.rs:95` - 抢占保护.
- `os/src/sync/per_cpu.rs:36` - per-CPU 数据.
- `os/src/arch/riscv/mm/page_table.rs:467` - RISC-V TLB 批处理.
- `os/src/arch/loongarch/mm/page_table.rs:440` - LoongArch64 TLB 批处理.
