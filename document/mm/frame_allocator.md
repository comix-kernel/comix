# 物理帧分配器

物理帧分配器管理可分配 DRAM 的页帧, 并把分配结果包装成 RAII tracker.它是页表页, 用户页, 内核按需页和连续 DMA 缓冲区的基础来源.

## 当前状态

- 全局分配器是 `FRAME_ALLOCATOR: SpinLock<FrameAllocator>`.
- 初始化范围来自 `mm::init()` 计算出的 `[start_ppn, end_ppn)`.
- 分配器使用位图记录每个物理页帧状态: bit 为 0 表示空闲, bit 为 1 表示已分配.
- 单页分配从 `last_alloc_hint` 附近开始扫描位图, 找到第一个空闲页后置位.
- 连续页分配扫描位图中的连续空闲区间, 因此释放后的连续空洞可以再次被复用.
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

1. 从 `last_alloc_hint` 指向的 bitmap word 开始循环扫描.
2. 跳过全满 word, 对非满 word 找第一个 0 bit.
3. 新 tracker 创建时清零整页.
4. 无页可用时返回 `None`.

### 回收

归还页会清除 bitmap 中对应 bit.调试构建下会检查页号范围和 double free.

### 连续分配

连续分配从 bitmap 起点扫描连续 0 bit.带对齐的连续分配只接受起始页号满足 `align_pages` 对齐的连续空闲区间.

## 并发与生命周期约束

- 全局入口都通过 `FRAME_ALLOCATOR.lock()` 串行化.
- tracker 不应被 `mem::forget` 泄漏, 否则对应物理页不会回收.
- `Ppn` 不是所有权凭据.只有 tracker 或拥有 tracker 的结构可以决定何时释放页.
- `MappingArea` 中的帧所有权必须随 VMA split/unmap/mprotect 一起移动或释放.

## 已知限制

- 位图容量目前按 8GiB 可管理物理内存静态预留, 避免帧分配器初始化早于堆初始化时依赖 `Vec`.
- 连续分配仍需线性扫描位图; 大内存和高碎片场景下成本高于伙伴系统.
- 调试检查依赖 `debug_assert!`, release 构建下不会阻止错误归还.

## 源码索引

- `os/src/mm/frame_allocator/mod.rs:15` - 公共分配入口.
- `os/src/mm/frame_allocator/allocator.rs:14` - `FrameTracker`.
- `os/src/mm/frame_allocator/allocator.rs:45` - `FrameRangeTracker`.
- `os/src/mm/frame_allocator/allocator.rs` - tracker 类型, 全局 `FRAME_ALLOCATOR`, bitmap 状态和分配/回收策略.
