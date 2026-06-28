# PreemptGuard

`PreemptGuard` 用 RAII 方式禁用抢占.它主要服务 `PerCpu<T>`: 访问当前 CPU 副本期间不能让任务迁移到其他 CPU.

## 当前状态

- 抢占计数是 per-CPU `PREEMPT_COUNT` 数组.
- 提供泛型版本 `preempt_disable_generic::<CPU>()` 等, 便于测试或显式架构选择.
- 生产路径 `PreemptGuard` 使用 `ArchImpl`.
- 支持嵌套, 计数大于 0 即视为抢占已禁用.

## 目标

- 防止访问 per-CPU 数据时任务迁移.
- 用 acquire/release fence 给临界区建立基本的编译器和 CPU 重排边界.
- 以 guard 形式减少手动 disable/enable 不配对的风险.

## 非目标

- 不屏蔽硬件中断.
- 不提供跨 CPU 互斥.
- 不负责调度器完整策略, 只维护当前 CPU 的抢占计数.

## 关键流程

1. `PreemptGuard::new()` 调用 `preempt_disable()`.
2. `preempt_disable()` 增加当前 CPU 的计数并执行 acquire fence.
3. guard drop 调用 `preempt_enable()`.
4. `preempt_enable()` 先执行 release fence, 再减少当前 CPU 的计数.

## 与 IntrGuard 的区别

- `IntrGuard` 处理本 CPU 中断重入.
- `PreemptGuard` 处理任务迁移.
- 访问 per-CPU 数据通常需要 `PreemptGuard`, 不一定需要关中断.
- 保护跨 CPU 共享数据需要锁, 不是单纯禁用抢占.

## 并发约束

- guard 必须在创建它的 CPU 上 drop.
- 计数没有下溢恢复机制, 手动 enable/disable 必须严格配对.
- 长时间禁用抢占会影响调度延迟.

## 已知限制

- 当前只维护计数, 调度器何时检查该计数由调度路径决定.
- 没有调试所有者或超时诊断.

## 源码索引

- `os/src/sync/preempt.rs:21` - `PREEMPT_COUNT`.
- `os/src/sync/preempt.rs:31` - 泛型 disable.
- `os/src/sync/preempt.rs:39` - 泛型 enable.
- `os/src/sync/preempt.rs:47` - 泛型状态检查.
- `os/src/sync/preempt.rs:53` - 泛型 guard.
- `os/src/sync/preempt.rs:78` - 生产 `preempt_disable()`.
- `os/src/sync/preempt.rs:95` - `PreemptGuard`.
