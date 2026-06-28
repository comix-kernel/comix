# 全局堆分配器

全局堆分配器为 `alloc` 生态提供内核动态内存.当前实现使用 `talc` 的 `Talck` 包装器, 锁类型是内核自己的 `RawSpinLock`.

## 当前状态

- 只有启用 `alloc` feature 时编译 `global_allocator` 实现.
- `#[global_allocator]` 是 `Talck<RawSpinLock, talc::ClaimOnOom>`.
- 初始 span 为空, `init_heap()` 在启动阶段用链接器符号 `sheap` 和 `eheap` claim 真实堆区.
- `RawSpinLock` 实现 `lock_api::RawMutex`, 因此 talc 可以通过同一套锁协议保护内部元数据.

## 目标

- 在 `no_std` 内核环境中支持 `Vec`, `Box`, `Arc`, `BTreeMap` 等动态分配.
- 复用 `RawSpinLock` 的中断保护, 避免在本地中断重入分配器时破坏 allocator 状态.
- 把堆区边界交给链接器脚本统一定义.

## 非目标

- 不提供用户态堆.用户态 `brk` 和 `mmap` 由 `MemorySpace` 管理.
- 不提供 per-CPU cache 或 slab allocator.
- 不把 OOM 恢复策略放在 MM 文档中展开.具体行为以 allocator 与 panic/OOM handler 源码为准.

## 初始化流程

1. `mm::init()` 在物理帧分配器初始化后调用 `init_heap()`.
2. `init_heap()` 读取 `sheap` 和 `eheap`.
3. 使用 talc `claim()` 把这段连续虚拟地址区声明为可分配堆.
4. 后续所有 `alloc` 分配通过全局 allocator 进入 talc.

## 并发与生命周期约束

- `init_heap()` 必须早于任何堆分配.
- `init_heap()` 只应在启动阶段调用一次.
- allocator 的锁保护范围应尽量短.持有 allocator 锁时不应主动触发可能再次分配的复杂路径.
- 中断保护只能解决本 CPU 中断重入问题, 跨 CPU 互斥仍由 `RawSpinLock` 原子状态保证.

## 已知限制

- 堆大小固定由链接器脚本给出, 当前没有向物理帧分配器动态扩容的路径.
- 没有独立的堆统计和碎片观测接口.
- 长时间持锁分配会影响中断延迟, 调用方应避免在中断上下文做复杂分配.

## 源码索引

- `os/src/mm/global_allocator/mod.rs:1` - feature gated 模块导出.
- `os/src/mm/global_allocator/talc_alloc.rs:11` - `RawSpinLock` 作为 allocator lock.
- `os/src/mm/global_allocator/talc_alloc.rs:22` - 全局 `Talck<RawSpinLock, ClaimOnOom>`.
- `os/src/mm/global_allocator/talc_alloc.rs:37` - `init_heap()` 读取链接器堆边界并 claim span.
