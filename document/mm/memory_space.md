# 地址空间管理

地址空间管理是 MM 子系统的策略层.它把页表后端, 物理帧所有权, VMA 元数据和系统调用语义组合成 `MemorySpace`.

## 当前状态

- `MemorySpace` 持有一个 `ActivePageTableInner`, 一个 `Vec<MappingArea>` 和用户堆起点 `heap_start`.
- `MappingArea` 持有 VMA 范围, 区域类型, 映射策略, 权限, 私有帧, 文件映射信息和共享内存段信息.
- 映射策略包括 `Direct`, `Framed`, `Reserved`, `Shared`.
- `memory_space` 当前按职责拆成:
  - `mapping_area/mod.rs` - VMA 元数据类型.
  - `mapping_area/map_ops.rs` - 单页/整区映射和复制数据.
  - `mapping_area/split_ops.rs` - fork 克隆, split, mprotect 局部修改, munmap 局部解除.
  - `mapping_area/resize_ops.rs` - brk 场景下尾部扩缩.
  - `mapping_area/file_ops.rs` - 文件映射加载和脏页写回.
  - `space/address_space.rs` - `MemorySpace` 基本操作和 fork.
  - `space/kernel_space.rs` - 内核地址空间和 MMIO 映射.
  - `space/elf_loader.rs` - ELF 用户程序装载.
  - `space/mmap_ops.rs` - `brk`, `mmap`, `munmap`, `mprotect`.

## 目标

- 用 VMA 记录虚拟地址空间布局, 用页表记录硬件映射.
- 支持内核共享映射和用户私有映射共存.
- 为 `brk`, `mmap`, `munmap`, `mprotect`, ELF 加载和 fork 提供一致的区域操作.
- 让帧所有权跟随 VMA 生命周期自动释放.

## 非目标

- 不实现完整 Linux VMA 红黑树或 mmap policy.
- fork 不是 COW, `Framed` 区域会深拷贝.
- 不在文档中列出所有 syscall 参数检查和错误分支, 这些细节看 rustdoc 和源码.

## 映射策略

### Direct

用于内核直接映射和内核段映射.`MappingArea` 不持有物理帧, 映射时从虚拟地址通过架构直接映射函数得到物理页.

### Framed

用于用户私有页和匿名映射.每个虚拟页分配 `FrameTracker`, tracker 放在 `MappingArea.frames` 中.VMA 被移除或拆分时, tracker 的所有权同步移动或释放.

### Reserved

用于占位但不可访问的区域, 例如 `PROT_NONE`.这种区域参与 VMA 重叠检查, 但不建立叶子 PTE.

### Shared

用于 SysV shared memory.VMA 保存共享段引用和段内页偏移, 映射时从共享段取 PPN, 不拥有私有帧.

## 关键流程

### 内核地址空间

`MemorySpace::new_kernel()` 创建空页表后调用 `map_kernel_space()`:

1. 按链接器符号映射 `.text`, `.rodata`, `.data`, `.bss.stack`, `.bss`, 堆.
2. 直接映射可用物理内存范围.RISC-V 覆盖设备树 DRAM, LoongArch64 对页表直映射窗口做 1GiB cap.
3. RISC-V 在需要时额外确保 DTB 所在页可访问.
4. MMIO 自动映射目前不是默认路径, 显式 MMIO 映射通过 `map_mmio` 系列接口管理.

### 用户地址空间

用户地址空间会复制当前内核映射的元数据并重新建立直接映射, 然后装入用户私有区域:

- `from_elf()` 解析 loadable segment, 建立 `Framed` 用户段.
- ET_DYN 使用固定 load bias, 并处理当前支持的最小重定位集合.
- 用户栈, sigreturn trampoline 和 heap 起点按内核配置设置.

### brk

`brk` 以 `heap_start` 为下界:

- 第一次扩展时创建 `UserHeap` 区域.
- 后续扩展只允许向未占用区域增长.
- 收缩会解除尾部映射, 收缩到起点则移除整个 heap VMA.

### mmap 与地址选择

匿名 `mmap` 在无 hint 时从用户堆顶和用户栈 guard 之间自顶向下找洞, 避免和向上增长的 brk 冲突.hint 会先向下页对齐, 如果冲突则回退到自动找洞.

### munmap 和 mprotect

`munmap` 和 `mprotect` 都先收集受影响 VMA 下标, 再倒序处理, 避免修改 `areas` 时下标失效.

- `munmap` 对中间区间解除映射时可能把一个 VMA 拆成左右两个 VMA.
- `mprotect(PROT_NONE)` 会把 `Framed` 区间转为 `Reserved` 并释放中间帧.
- 从 `Reserved` 改回可访问权限会重新分配帧并建立映射.
- `Direct` 映射不允许通过用户 mprotect 路径修改.

### fork

`clone_for_fork()` 按映射策略处理:

- `Direct` 只克隆元数据并重新映射.
- `Framed` 分配新帧并复制页内容.
- `Reserved` 只克隆元数据.
- `Shared` 复制共享段引用并重新建立共享映射.

## 并发与生命周期约束

- 进程级 `MemorySpace` 通常由外层锁保护.文档不假设 `MemorySpace` 本身可无锁并发修改.
- `MappingArea.frames` 是私有帧所有权边界.split, mprotect, munmap 必须移动或释放 tracker, 不能只改页表.
- 文件映射在 `munmap` 前和 `MemorySpace::drop()` 时尽力写回脏页.
- 页表修改经 `map_with_batch`, `unmap_with_batch`, `update_flags_with_batch` 进入架构 TLB 刷新策略.

## 已知限制

- VMA 容器是线性 `Vec`, 地址空间碎片多时查找成本会上升.
- 文件映射和共享映射能力仍是基础实现, 与 Linux 完整 mmap 语义存在差距.
- `mmap` hint 冲突时不会做复杂的邻近搜索.
- 当前没有完整 COW, fork 成本随私有页数量线性增长.

## 源码索引

- `os/src/mm/memory_space/mod.rs:1` - 模块组织和重导出.
- `os/src/mm/memory_space/mmap_file.rs:8` - 文件映射元数据.
- `os/src/mm/memory_space/mapping_area/mod.rs:16` - `MapType`, `AreaType`, `MappingArea`.
- `os/src/mm/memory_space/mapping_area/map_ops.rs:86` - 单页和整区映射.
- `os/src/mm/memory_space/mapping_area/split_ops.rs:4` - VMA 元数据克隆和 fork 数据复制.
- `os/src/mm/memory_space/mapping_area/split_ops.rs:234` - `mprotect` 局部权限修改.
- `os/src/mm/memory_space/mapping_area/split_ops.rs:444` - `munmap` 局部解除映射.
- `os/src/mm/memory_space/mapping_area/resize_ops.rs:8` - 区域尾部扩缩.
- `os/src/mm/memory_space/mapping_area/file_ops.rs:9` - 文件映射读入.
- `os/src/mm/memory_space/mapping_area/file_ops.rs:73` - 文件映射写回.
- `os/src/mm/memory_space/space/address_space.rs:3` - `MemorySpace` 基本操作.
- `os/src/mm/memory_space/space/kernel_space.rs:30` - 内核空间构建.
- `os/src/mm/memory_space/space/elf_loader.rs:22` - ELF 装载.
- `os/src/mm/memory_space/space/mmap_ops.rs:10` - 用户内存系统调用支持.
