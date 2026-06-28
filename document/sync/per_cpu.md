# PerCpu

`PerCpu<T>` 为每个 CPU 保存一份独立数据.它用空间换取低竞争访问, 适合统计计数器, 当前 CPU 缓存和 CPU 本地状态.

## 当前状态

- 数据存储为 `Vec<CacheAligned<T>>`.
- 每个元素用 64 字节对齐减少 false sharing.
- CPU ID 来自 `CpuOps::id()`.
- `get()` 和 `get_mut()` 访问当前 CPU 副本.
- `get_of(cpu_id)` 读取指定 CPU 副本.

## 目标

- 避免所有 CPU 频繁争用同一把锁.
- 给 CPU 本地数据提供简单容器.
- 和 `PreemptGuard` 配合保证访问当前 CPU 副本期间不会迁移.

## 非目标

- 不自动禁用抢占.
- 不提供跨 CPU 汇总一致性快照.
- 不保证 `T` 内部操作原子.需要跨 CPU 读取或汇总时, `T` 自身仍可能需要原子类型或锁.

## 关键流程

### 创建

`PerCpu::new()` 从 `kernel::num_cpu()` 获取 CPU 数量, 为每个 CPU 调用初始化闭包.`new_with_id()` 把 CPU ID 传给闭包.测试或特殊路径可用 `new_with_id_and_count()` 指定数量.

### 当前 CPU 访问

`get()` 和 `get_mut()` 使用当前 CPU ID 索引数据.`get_mut()` 从共享引用返回可变引用, 因此调用方必须保证当前任务不会迁移, 且同一 CPU 上没有并发可变访问.

### 指定 CPU 读取

`get_of(cpu_id)` 只做范围检查并返回指定副本引用.它不提供快照一致性, 适合诊断或调用方已有同步的场景.

## 并发与生命周期约束

- 访问当前 CPU 可变副本前应持有 `PreemptGuard`.
- 如果中断处理程序也访问同一 per-CPU 数据, 还需要考虑中断屏蔽或内部原子性.
- `PerCpu<T>` 的 `Send`/`Sync` 约束要求 `T: Send`, 但这不代表任意访问模式都无竞争.
- CPU 数量必须在创建前初始化, 否则会断言失败.

## 已知限制

- CPU 热插拔不在当前设计内.
- 数据副本数量创建后固定.
- 没有内置遍历汇总 API.

## 源码索引

- `os/src/sync/per_cpu.rs:16` - cache line 对齐包装.
- `os/src/sync/per_cpu.rs:36` - `PerCpu<T>`.
- `os/src/sync/per_cpu.rs:42` - `new()`.
- `os/src/sync/per_cpu.rs:56` - `new_with_id()`.
- `os/src/sync/per_cpu.rs:70` - `new_with_id_and_count()`.
- `os/src/sync/per_cpu.rs:85` - 当前 CPU 只读访问.
- `os/src/sync/per_cpu.rs:93` - 当前 CPU 可变访问.
- `os/src/sync/per_cpu.rs:100` - 指定 CPU 访问.
