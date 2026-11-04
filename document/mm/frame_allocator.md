# 物理帧分配器

物理帧分配器管理可分配 DRAM 的页帧, 并把分配结果包装成 RAII tracker.它是页表页, 用户页, 内核按需页和连续 DMA 缓冲区的基础来源.

## 当前状态

- 全局分配器是 `FRAME_ALLOCATOR: SpinLock<FrameAllocator>`.
- `mm::init()` 可以交给分配器最多 8 段已排序、互不重叠的 `FrameRegion`. 设备树里不连续的 DRAM 会保留成多段, 中间的洞不进入位图.
- 位图按下标记录可用帧, 不按物理地址跨度记录. bit 为 0 表示空闲, bit 为 1 表示已分配. 下标通过区域表换回真实 `Ppn`.
- 单页分配从 `last_alloc_hint` 附近开始扫描位图, 找到第一个空闲页后置位.
- 连续页分配只在同一段 `FrameRegion` 内扫描连续空闲下标, 因此释放后的连续空洞可以再次被复用, 但不会把两段 DRAM 之间的物理空洞当成连续内存.
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

`init_frame_allocator_regions` 接收一组 `FrameRegion`. 单区间入口 `init_frame_allocator(start_addr, end_addr)` 只是把它包成一段后再走同一条路径.

`mm::init()` 先把 `ekernel` 换成页对齐物理地址, 再把设备树 DRAM 裁到这个起点之后、且不超过起点后 1GiB. 没有任何可用区间时, 回退到编译期 `MEMORY_END` 与同一上限之间的单段. 每段起点用 ceil, 终点用 floor, 避免吃进不完整页.

区域必须按起始页号排序且不重叠. 位图长度是各段页数之和, 上限对应 8GiB 可管理帧, 避免帧分配器初始化早于堆时依赖 `Vec`.

### 单页分配

1. 从 `last_alloc_hint` 指向的 bitmap word 开始循环扫描.
2. 跳过全满 word, 对非满 word 找第一个 0 bit.
3. 新 tracker 创建时清零整页.
4. 无页可用时返回 `None`.

### 回收

归还页会清除 bitmap 中对应 bit.调试构建下会检查页号范围和 double free.

### 连续分配

连续分配从 bitmap 起点扫描连续 0 bit, 并且扫描不能越过当前 `FrameRegion` 的结束下标. 一段的末尾即使紧挨着下一段的起始下标, 也不视为物理连续. 带对齐的连续分配只接受起始页号满足 `align_pages` 对齐、并且整段都落在同一区域里的空闲区间.

## 并发与生命周期约束

- 全局入口都通过 `FRAME_ALLOCATOR.lock()` 串行化.
- tracker 不应被 `mem::forget` 泄漏, 否则对应物理页不会回收.
- `Ppn` 不是所有权凭据.只有 tracker 或拥有 tracker 的结构可以决定何时释放页.
- `MappingArea` 中的帧所有权必须随 VMA split/unmap/mprotect 一起移动或释放.

## 已知限制

- 早期可用内存另外被限制在内核镜像结束后的 1GiB 内. 8GiB 是位图能记录的帧数上限, 不是当前一定会交给分配器的物理跨度.
- 连续分配仍需线性扫描位图; 大内存和高碎片场景下成本高于伙伴系统.
- 区域之间的物理空洞不能用于连续 DMA 或大块连续分配.
- 调试检查依赖 `debug_assert!`, release 构建下不会阻止错误归还.

## 源码索引

- `os/src/mm/mod.rs` - 从设备树裁剪 DRAM 区间并调用区域初始化.
- `os/src/mm/frame_allocator/mod.rs` - `init_frame_allocator` 与 `init_frame_allocator_regions`.
- `os/src/mm/frame_allocator/allocator.rs` - `FrameRegion`, `FrameTracker`, `FrameRangeTracker` 和位图分配.
