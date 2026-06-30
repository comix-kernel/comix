# MM 子系统文档

MM 子系统负责 Comix 内核的物理帧分配, 内核堆, 页表抽象和进程地址空间管理. 当前实现把架构无关的策略放在 `os/src/mm/`, 把页表格式, TLB 刷新和地址转换放在 `os/src/arch/{riscv,loongarch}/mm/`.

本文档聚焦设计边界和关键流程. 具体函数签名, 字段和错误分支以 rustdoc 与源码为准.

## 当前状态

- 支持 RISC-V 和 LoongArch64 的页表后端.
- 地址和页号使用强类型包装: `PA`, `VA`, `UA`, `Ppn`, `Vpn`.
- 物理帧由全局 `SpinLock<FrameAllocator>` 保护, 已分配帧通过 RAII tracker 回收.
- 全局堆使用 `talc::Talck<RawSpinLock, ClaimOnOom>`.
- 地址空间由 `MemorySpace` 维护页表和 `MappingArea` 列表, 支持 `brk`, `mmap`, `munmap`, `mprotect`, ELF 加载, fork 克隆, 文件映射和 SysV shared memory 映射.
- TLB 批处理上下文由架构后端提供: RISC-V 会合并跨核 shootdown, LoongArch 当前只保证本地刷新.

## 模块边界

```text
os/src/mm/
+-- mod.rs
+-- address/
|   +-- mod.rs
|   +-- operations.rs
|   +-- page_num.rs
|   +-- types.rs
+-- frame_allocator/
|   +-- mod.rs
|   +-- allocator.rs
+-- global_allocator/
|   +-- mod.rs
|   +-- talc_alloc.rs
+-- page_table/
|   +-- mod.rs
|   +-- inner.rs
|   +-- page_table_entry.rs
+-- memory_space/
    +-- mod.rs
    +-- mmap_file.rs
    +-- mapping_area/
    |   +-- mod.rs
    |   +-- map_ops.rs
    |   +-- split_ops.rs
    |   +-- resize_ops.rs
    |   +-- file_ops.rs
    +-- space/
        +-- mod.rs
        +-- address_space.rs
        +-- kernel_space.rs
        +-- elf_loader.rs
        +-- mmap_ops.rs
        +-- tests.rs
```

架构相关目录:

```text
os/src/arch/riscv/mm/
+-- mod.rs
+-- page_table.rs
+-- page_table_entry.rs

os/src/arch/loongarch/mm/
+-- mod.rs
+-- page_table.rs
+-- page_table_entry.rs
```

## 初始化流程

`mm::init()` 是启动入口:

1. 把链接器符号 `ekernel` 从内核虚拟地址转换为物理地址, 并按页向上对齐.
2. 优先从设备树读取真实 DRAM 范围, 读取失败时回退到编译期 `MEMORY_END`.
3. LoongArch64 对可用物理范围设 1GiB 上限, 与内核直接映射窗口保持一致; RISC-V 覆盖设备树报告的 DRAM.
4. 初始化物理帧分配器.
5. 在启用 `alloc` 时初始化 talc 堆.
6. 创建最终内核 `MemorySpace`, 保存到全局内核空间句柄, 供其他 CPU 使用同一份内核页表.

`mm::init()` 创建但不主动激活页表. 激活由调用者在合适的启动阶段通过 `mm::activate(root_ppn)` 完成.

## 核心设计约定

- 所有 `Range`, `AddressRange`, `PageNumRange`, `VpnRange`, `PpnRange` 都采用 `[start, end)` 语义.
- 内核映射和用户映射共享同一页表结构: 用户页表包含用户私有映射和内核共享映射, 通过 PTE 的用户访问位隔离权限.
- `MappingArea` 是 VMA 级别的元数据和帧所有权边界. 页表只保存硬件可见映射, 不负责记录映射来源.
- `MapType::Reserved` 表示占用虚拟地址但不建立叶子 PTE, 用于 `PROT_NONE` 和 guard page.
- 当前页表路径只启用 4K 页. `PageSize` 目前只有 `Size4K`.
- 架构后端必须把 `UniversalPTEFlag` 翻译成自己的 PTE 标志, 不能把 RISC-V 位语义直接泄漏到上层.

## 文档导航

- [整体架构](architecture.md)
- [地址抽象层](address.md)
- [物理帧分配器](frame_allocator.md)
- [全局堆分配器](global_allocator.md)
- [页表抽象层](page_table.md)
- [地址空间管理](memory_space.md)
- [源码索引](api_reference.md)

## 已知限制

- `MemorySpace::areas` 仍是线性 `Vec`, 区域查找和重叠检测是线性扫描.
- fork 当前对 `Framed` 区域做深拷贝, 不是真正 COW.
- RISC-V 后端有跨核 TLB shootdown, 但 LoongArch 后端的批处理上下文当前是本地刷新占位实现.
- 4K 页是唯一启用路径, 大页映射还未作为正式能力暴露.

## 源码索引

- `os/src/mm/mod.rs:36` - MM 初始化, DRAM 范围选择, 内核空间全局句柄.
- `os/src/mm/address/` - 地址, 页号, 范围和转换 trait.
- `os/src/mm/frame_allocator/allocator.rs:96` - 全局帧分配器和 RAII tracker.
- `os/src/mm/global_allocator/talc_alloc.rs:22` - talc 全局堆分配器.
- `os/src/mm/page_table/inner.rs:8` - 架构页表 trait.
- `os/src/mm/page_table/page_table_entry.rs:12` - 通用 PTE 标志和转换接口.
- `os/src/mm/memory_space/mapping_area/mod.rs:16` - VMA 类型, 映射策略和所有权字段.
- `os/src/mm/memory_space/space/address_space.rs:3` - `MemorySpace` 基本操作和 fork 克隆.
- `os/src/mm/memory_space/space/kernel_space.rs:30` - 内核映射构建.
- `os/src/mm/memory_space/space/mmap_ops.rs:10` - `brk`, `mmap`, `munmap`, `mprotect`.
