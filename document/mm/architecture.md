# MM 子系统整体架构

MM 子系统采用三层结构: 类型和资源所有权在通用 MM 层, 硬件页表在架构层, 进程语义在 `MemorySpace` 层.

## 分层设计

```text
系统调用和进程管理
        |
        v
MemorySpace
  - VMA 列表
  - brk/mmap/munmap/mprotect
  - ELF/fork/file/shared mapping
        |
        v
MappingArea
  - 映射策略
  - 帧所有权
  - 文件和共享段元数据
        |
        v
PageTableInner trait + UniversalPTEFlag
        |
        v
arch/riscv/mm 或 arch/loongarch/mm
  - PTE 格式
  - 页表 walker
  - activate
  - TLB flush
        |
        v
FrameAllocator + talc heap + address types
```

## 关键设计

### 地址类型先于页表

MM 代码不直接传递裸整数表示地址.`PA`, `VA`, `UA`, `Ppn`, `Vpn` 把"这个数是什么"写进类型系统.这样页表, 帧分配器和地址空间管理可以共享对齐和范围语义.

### 页表后端只做硬件映射

页表后端负责从 VPN 到 PPN 的硬件可见映射, 不记录映射来自匿名页, 文件页还是共享内存.来源和所有权在 `MappingArea` 中维护.

### VMA 是所有权边界

`MappingArea` 同时记录虚拟范围和映射策略.对于 `Framed` 区域, 它还拥有物理帧 tracker.拆分, 解除映射和权限修改必须先处理 VMA 所有权, 再处理页表.

### 内核映射被复制到用户页表

用户地址空间包含用户私有区域和内核共享区域.陷入内核时不需要切换到另一张内核页表, 但用户态不能访问 U=0 的内核映射.

### PROT_NONE 不是无权限叶子 PTE

RISC-V 不接受没有 R/W/X 的普通叶子 PTE 作为可访问映射.当前设计用 `MapType::Reserved` 表示地址占位, 避免在硬件页表中创建这种条目.

## 初始化顺序

1. `mm::init()` 计算可用物理内存范围.
2. 初始化全局帧分配器.
3. 初始化 talc 堆.
4. 创建最终内核 `MemorySpace`.
5. 保存全局内核空间句柄.
6. 调用方在合适阶段激活根页表.

这个顺序保证页表创建所需的物理帧已可分配, 堆分配器可服务后续动态结构, 多 CPU 最终共享 CPU0 建好的内核映射.

## 架构扩展边界

新增架构需要提供:

- 地址类型和直接映射转换函数.
- `PageTableInner` 实现.
- `PageTableEntry` 实现.
- `UniversalPTEFlag` 到架构 PTE flags 的转换.
- 页表激活和 TLB 刷新机制.

通用层不应依赖某个后端的 CSR, PTE 位布局或 TLB 指令.

## 并发与生命周期

- 全局帧分配器由 `SpinLock` 保护.
- 全局堆由 `RawSpinLock` 保护.
- 全局内核空间句柄由 `SpinLock<Option<Arc<SpinLock<MemorySpace>>>>` 保存.
- 进程地址空间由外层内核对象负责加锁; `MemorySpace` 修改路径自身不是无锁并发结构.
- 页表页和普通物理页都通过 RAII tracker 释放, 但它们的所有者不同: 页表后端拥有页表页, VMA 拥有用户数据页.

## 性能取舍

- 线性 VMA 列表实现简单, 但大量映射下查找成本高.
- fork 深拷贝简单可靠, 但比 COW 更耗时和耗内存.
- RISC-V TLB 批处理减少 IPI 数量, LoongArch64 仍是较保守的本地刷新.
- 连续物理帧分配不做复杂碎片整理, 保持实现简单.

## 已知限制

- 大页, COW, VMA 树, per-CPU frame cache 和可增长内核堆都不是当前正式能力.
- LoongArch64 和 RISC-V 的 TLB shootdown 能力不完全对齐.
- 设备树 DRAM 信息缺失时仍会回退到编译期 `MEMORY_END`.

## 源码索引

- `os/src/mm/mod.rs:36` - 初始化编排.
- `os/src/mm/address/` - 地址与页号类型系统.
- `os/src/mm/frame_allocator/allocator.rs:100` - 帧分配器状态.
- `os/src/mm/global_allocator/talc_alloc.rs:22` - talc 堆.
- `os/src/mm/page_table/inner.rs:8` - 页表 trait 边界.
- `os/src/mm/memory_space/mapping_area/mod.rs:52` - VMA 所有权边界.
- `os/src/mm/memory_space/space/address_space.rs:338` - fork 克隆策略.
- `os/src/mm/memory_space/space/kernel_space.rs:30` - 内核映射策略.
- `os/src/arch/riscv/mm/page_table.rs:21` - RISC-V 页表后端.
- `os/src/arch/loongarch/mm/page_table.rs:34` - LoongArch64 页表后端.
