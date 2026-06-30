# IntrGuard

`IntrGuard` 是本 CPU 中断屏蔽的 RAII guard.它解决的是本 CPU 上"任务代码被中断处理程序打断"造成的重入问题, 不是跨 CPU 互斥.

## 当前状态

- 通过 `CpuOps` 禁用和恢复中断.
- 使用 per-CPU 嵌套深度 `INTR_DEPTH`.
- 只有最外层 guard 保存进入前 flags 并在 drop 时恢复.
- 内层 guard drop 不会提前打开中断.

## 目标

- 为 `RawSpinLock`, `SpinLock`, `RwLock` 提供本 CPU 中断保护.
- 允许嵌套使用而不破坏外层临界区.
- 用 RAII 避免异常返回路径遗漏恢复中断状态.

## 非目标

- 不阻止其他 CPU 并发访问同一数据.
- 不表示抢占禁用语义.访问 per-CPU 数据应使用 `PreemptGuard` 或等价机制.
- 不应包裹长时间运行或可能调度的代码.

## 关键流程

1. 读取当前 CPU 的中断保护深度.
2. 如果深度为 0, 调用 `CPU::disable_interrupts()` 并保存 flags.
3. 增加 per-CPU 深度并建立 acquire fence.
4. drop 时建立 release fence, 减少深度.
5. 如果 drop 的是最外层 guard, 恢复保存的 flags.

## 并发约束

- guard 必须在创建它的 CPU 上 drop.迁移会破坏 per-CPU 深度和 flags 对应关系.
- 嵌套 guard 的 `was_enabled()` 返回 false, 因为它进入时中断已经由外层关闭.
- `IntrGuard` 可组合原子锁形成 SMP 安全锁, 但单独使用只适合 CPU 本地临界区.

## 已知限制

- 深度数组大小由 `MAX_CPU_COUNT` 固定.
- 不记录调用栈或所有者, 不能诊断中断长期关闭的来源.

## 源码索引

- `os/src/sync/intr_guard.rs:23` - per-CPU 缓存行对齐计数器.
- `os/src/sync/intr_guard.rs:33` - `INTR_DEPTH`.
- `os/src/sync/intr_guard.rs:37` - `SAVED_INTR_FLAGS`.
- `os/src/sync/intr_guard.rs:44` - `IntrGuard`.
- `os/src/sync/intr_guard.rs:55` - 创建和嵌套处理.
- `os/src/sync/intr_guard.rs:85` - drop 恢复中断状态.
