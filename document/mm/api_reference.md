# MM 源码索引

本页不维护完整 API 清单.公共函数签名, 参数和错误分支以 rustdoc 与源码为准.这里按设计职责索引源码入口, 方便从文档跳到实现.

## 初始化

- `os/src/mm/mod.rs:36` - `mm::init()`, 负责帧分配器, 堆和内核地址空间创建.
- `os/src/mm/mod.rs:131` - `mm::activate(root_ppn)`, 通过架构页表后端激活地址空间.
- `os/src/mm/mod.rs:145` - 全局内核空间句柄, 供多 CPU 使用最终内核页表.

## 地址和页号

- `os/src/mm/address/types.rs:12` - `Address` trait.
- `os/src/mm/address/types.rs:103` - 直接映射地址转换 trait.
- `os/src/mm/address/types.rs:132` - `AddressRange<T>`.
- `os/src/mm/address/page_num.rs:13` - `PageNum` trait.
- `os/src/mm/address/page_num.rs:104` - `Ppn` 和 `Vpn`.
- `os/src/mm/address/page_num.rs:116` - `PageNumRange<T>`.
- `os/src/mm/address/operations.rs:12` - `UsizeConvert` 与算术/对齐 trait.

## 物理帧

- `os/src/mm/frame_allocator/mod.rs:15` - 对外分配入口.
- `os/src/mm/frame_allocator/allocator.rs:14` - 单帧 RAII tracker.
- `os/src/mm/frame_allocator/allocator.rs:45` - 连续帧 RAII tracker.
- `os/src/mm/frame_allocator/allocator.rs:96` - 全局 `FRAME_ALLOCATOR`.
- `os/src/mm/frame_allocator/allocator.rs:100` - `FrameAllocator` 状态.
- `os/src/mm/frame_allocator/allocator.rs:124` - 单帧分配.
- `os/src/mm/frame_allocator/allocator.rs:143` - 多个非连续帧分配.
- `os/src/mm/frame_allocator/allocator.rs:163` - 连续帧分配.
- `os/src/mm/frame_allocator/allocator.rs:180` - 对齐连续帧分配.
- `os/src/mm/frame_allocator/allocator.rs:222` - 单帧回收.
- `os/src/mm/frame_allocator/allocator.rs:259` - 连续帧回收.

## 全局堆

- `os/src/mm/global_allocator/mod.rs:1` - feature gated 导出.
- `os/src/mm/global_allocator/talc_alloc.rs:22` - `Talck<RawSpinLock, ClaimOnOom>` 全局 allocator.
- `os/src/mm/global_allocator/talc_alloc.rs:37` - `init_heap()`.

## 页表通用接口

- `os/src/mm/page_table/mod.rs:11` - 活动页表类型别名.
- `os/src/mm/page_table/mod.rs:15` - `PageSize`.
- `os/src/mm/page_table/mod.rs:21` - `PagingError`.
- `os/src/mm/page_table/inner.rs:8` - `PageTableInner` trait.
- `os/src/mm/page_table/page_table_entry.rs:12` - `UniversalPTEFlag`.
- `os/src/mm/page_table/page_table_entry.rs:48` - 通用 flag 构造辅助.
- `os/src/mm/page_table/page_table_entry.rs:61` - `UniversalConvertableFlag`.
- `os/src/mm/page_table/page_table_entry.rs:70` - `PageTableEntry` trait.

## 页表架构后端

- `os/src/arch/riscv/mm/page_table.rs:13` - RISC-V 页表状态和 root frame 所有权.
- `os/src/arch/riscv/mm/page_table.rs:21` - SV39 trait 实现.
- `os/src/arch/riscv/mm/page_table.rs:403` - 带批处理的 map.
- `os/src/arch/riscv/mm/page_table.rs:425` - 带批处理的 unmap.
- `os/src/arch/riscv/mm/page_table.rs:444` - 带批处理的权限更新.
- `os/src/arch/riscv/mm/page_table.rs:467` - RISC-V `TlbBatchContext`.
- `os/src/arch/riscv/mm/page_table_entry.rs:7` - SV39 PTE flags.
- `os/src/arch/loongarch/mm/page_table.rs:25` - LoongArch64 页表状态.
- `os/src/arch/loongarch/mm/page_table.rs:34` - LoongArch64 trait 实现.
- `os/src/arch/loongarch/mm/page_table.rs:399` - 带批处理的 map.
- `os/src/arch/loongarch/mm/page_table.rs:413` - 带批处理的 unmap.
- `os/src/arch/loongarch/mm/page_table.rs:424` - 带批处理的权限更新.
- `os/src/arch/loongarch/mm/page_table.rs:440` - LoongArch64 `TlbBatchContext`.
- `os/src/arch/loongarch/mm/page_table_entry.rs:35` - LoongArch64 PTE flags.

## 地址空间和 VMA

- `os/src/mm/memory_space/mod.rs:1` - 模块组织.
- `os/src/mm/memory_space/mmap_file.rs:8` - 文件映射元数据.
- `os/src/mm/memory_space/mapping_area/mod.rs:16` - `MapType`.
- `os/src/mm/memory_space/mapping_area/mod.rs:33` - `AreaType`.
- `os/src/mm/memory_space/mapping_area/mod.rs:52` - `MappingArea`.
- `os/src/mm/memory_space/mapping_area/map_ops.rs:50` - 创建 VMA.
- `os/src/mm/memory_space/mapping_area/map_ops.rs:86` - 单页映射.
- `os/src/mm/memory_space/mapping_area/map_ops.rs:137` - 整区映射.
- `os/src/mm/memory_space/mapping_area/map_ops.rs:150` - 单页解除映射.
- `os/src/mm/memory_space/mapping_area/map_ops.rs:178` - 整区解除映射.
- `os/src/mm/memory_space/mapping_area/map_ops.rs:191` - 已映射区域数据复制.
- `os/src/mm/memory_space/mapping_area/split_ops.rs:4` - 元数据克隆.
- `os/src/mm/memory_space/mapping_area/split_ops.rs:26` - 帧映射深拷贝.
- `os/src/mm/memory_space/mapping_area/split_ops.rs:121` - VMA 拆分.
- `os/src/mm/memory_space/mapping_area/split_ops.rs:217` - 局部权限修改.
- `os/src/mm/memory_space/mapping_area/split_ops.rs:429` - 局部解除映射.
- `os/src/mm/memory_space/mapping_area/resize_ops.rs:8` - 尾部扩展.
- `os/src/mm/memory_space/mapping_area/resize_ops.rs:31` - 尾部收缩.
- `os/src/mm/memory_space/mapping_area/file_ops.rs:9` - 文件映射读入.
- `os/src/mm/memory_space/mapping_area/file_ops.rs:73` - 文件映射写回.

## MemorySpace

- `os/src/mm/memory_space/space/mod.rs:38` - 内核 token/root 辅助.
- `os/src/mm/memory_space/space/mod.rs:58` - `MemorySpace` 字段.
- `os/src/mm/memory_space/space/address_space.rs:3` - 空地址空间创建.
- `os/src/mm/memory_space/space/address_space.rs:52` - 创建带内核映射的用户空间.
- `os/src/mm/memory_space/space/address_space.rs:208` - 插入 VMA 并检查重叠.
- `os/src/mm/memory_space/space/address_space.rs:338` - fork 克隆.
- `os/src/mm/memory_space/space/address_space.rs:379` - drop 时写回文件映射.
- `os/src/mm/memory_space/space/kernel_space.rs:30` - 内核映射构建.
- `os/src/mm/memory_space/space/kernel_space.rs:175` - 创建内核空间.
- `os/src/mm/memory_space/space/kernel_space.rs:217` - 映射 MMIO 区域.
- `os/src/mm/memory_space/space/kernel_space.rs:284` - 解除 MMIO 映射.
- `os/src/mm/memory_space/space/elf_loader.rs:22` - ELF 装载.
- `os/src/mm/memory_space/space/mmap_ops.rs:10` - `brk`.
- `os/src/mm/memory_space/space/mmap_ops.rs:104` - 用户 mmap 空洞搜索.
- `os/src/mm/memory_space/space/mmap_ops.rs:205` - `mmap`.
- `os/src/mm/memory_space/space/mmap_ops.rs:291` - `munmap`.
- `os/src/mm/memory_space/space/mmap_ops.rs:375` - `mprotect`.
