# 物理帧分配器

物理帧分配器管理可分配 DRAM 的页帧, 并把分配结果包装成 RAII tracker.它是页表页, 用户页, 内核按需页和连续 DMA 缓冲区的基础来源.

## 当前状态

- 全局分配器是 `FRAME_ALLOCATOR: SpinLock<FrameAllocator>`.
- 初始化范围来自 `mm::init()` 计算出的 `[start_ppn, end_ppn)`.
- 单页分配优先使用回收栈, 回收栈为空时从水位线 `cur` 向上分配.
- 连续页分配只从水位线之后的连续区域分配, 不在回收栈中做复杂拼接.
- `FrameTracker` 和 `FrameRangeTracker` 在创建时清零物理页, 在 drop 时自动归还.

## 目标

- 提供早期内核可用的简单物理帧管理.
- 用所有权表达"谁负责归还物理页".
- 支持单页, 多个非连续页, 连续页和带页数对齐的连续页.

## 非目标

- 不提供伙伴系统或 slab 级别的长期碎片治理.
- 不维护每页引用计数, 也不实现 COW.
- 不处理 NUMA, zone, DMA mask 或 cache attribute.

## 关键流程

### 初始化

`init_frame_allocator(start_addr, end_addr)` 把物理地址转换成页号范围:

- 起点用 ceil, 避免分配覆盖内核镜像尾部的非完整页.
- 终点用 floor, 避免分配超出可用物理内存尾部的非完整页.

### 单页分配

1. 先从 `recycled` 弹出被归还的页.
2. 如果没有可回收页, 且 `cur < end`, 使用 `cur` 并前移水位线.
3. 新 tracker 创建时清零整页.
4. 无页可用时返回 `None`.

### 回收与合并

归还页会被加入 `recycled` 并排序.如果回收栈末尾正好贴近 `cur`, 分配器会把连续尾部并回水位线, 让未来连续分配重新利用这段空间.

### 连续分配

连续分配只检查当前水位线之后是否有足够页.带对齐的连续分配会把水位线向上对齐, 中间跳过的页加入回收栈.

## 并发与生命周期约束

- 全局入口都通过 `FRAME_ALLOCATOR.lock()` 串行化.
- tracker 不应被 `mem::forget` 泄漏, 否则对应物理页不会回收.
- `Ppn` 不是所有权凭据.只有 tracker 或拥有 tracker 的结构可以决定何时释放页.
- `MappingArea` 中的帧所有权必须随 VMA split/unmap/mprotect 一起移动或释放.

## 已知限制

- 回收栈排序是简单实现, 在大量回收时成本会升高.
- 连续分配不从回收栈组合碎片, 长时间运行后连续大块可能更难获得.
- 调试检查依赖 `debug_assert!`, release 构建下不会阻止错误归还.

## 源码索引

- `os/src/mm/frame_allocator/mod.rs:15` - 公共分配入口.
- `os/src/mm/frame_allocator/allocator.rs:14` - `FrameTracker`.
- `os/src/mm/frame_allocator/allocator.rs:45` - `FrameRangeTracker`.
- `os/src/mm/frame_allocator/allocator.rs:96` - 全局 `FRAME_ALLOCATOR`.
- `os/src/mm/frame_allocator/allocator.rs:100` - `FrameAllocator` 状态.
- `os/src/mm/frame_allocator/allocator.rs:124` - 单页分配策略.
- `os/src/mm/frame_allocator/allocator.rs:163` - 连续帧分配.
- `os/src/mm/frame_allocator/allocator.rs:222` - 回收与尾部合并.
