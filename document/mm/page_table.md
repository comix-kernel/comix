# 页表抽象层

页表抽象层把 MM 策略和硬件页表格式隔离开.上层只处理 `Vpn`, `Ppn`, `PageSize` 和 `UniversalPTEFlag`, 具体页表级数, PTE 位布局, 激活寄存器和 TLB 刷新由架构后端实现.

## 当前状态

- 架构无关接口位于 `os/src/mm/page_table/`.
- 当前活动页表类型别名是 `ActivePageTableInner = crate::arch::mm::PageTableInner`.
- RISC-V 后端实现 SV39 三级页表.
- LoongArch64 后端实现 4 级页表, 匹配 48 位虚拟地址.
- 当前只启用 4K 页路径, `PageSize` 只有 `Size4K`.

## 目标

- 统一上层页表操作接口.
- 让 `MemorySpace` 不依赖某个架构的 PTE 格式.
- 把跨 CPU TLB 刷新策略留给架构后端.
- 用 `UniversalPTEFlag` 表达用户/内核权限和 R/W/X/D 等通用含义.

## 非目标

- 不在通用层承诺每个架构标志位一一对应.
- 不提供大页稳定接口.
- 不在通用层处理 ASID 生命周期.

## 模块边界

- `page_table/inner.rs` 定义 `PageTableInner<T: PageTableEntry>`.
- `page_table/page_table_entry.rs` 定义通用 PTE flag, flag 转换 trait 和 PTE trait.
- `arch/riscv/mm/page_table.rs` 负责 SV39 页表创建, walk, map, unmap, update flags, satp 激活和 TLB shootdown.
- `arch/riscv/mm/page_table_entry.rs` 负责 SV39 PTE 位布局.
- `arch/loongarch/mm/page_table.rs` 负责 LoongArch64 页表创建, CSR 配置, PGDL/PGDH 激活和本地 TLB 刷新.
- `arch/loongarch/mm/page_table_entry.rs` 负责 LoongArch PTE 位布局和反逻辑 NR/NX 翻译.

## 关键流程

### 建立映射

1. 上层传入 VPN, PPN, 4K 页大小和通用权限.
2. 后端校验叶子 PTE 至少具备 R/W/X 之一.`PROT_NONE` 不应走叶子 PTE, 上层用 `MapType::Reserved` 表示.
3. 页表 walk 从根层向下查找, 必要时分配中间页表帧.
4. 叶子 PTE 写入架构格式.
5. 后端刷新本地 TLB, RISC-V 在非批处理模式下通知其他 CPU.

### 解除映射

后端 walk 到叶子 PTE 后清空条目.中间页表帧当前由 `PageTableInner.frames` 持有并随整个页表释放, 不在每次 unmap 时做空表回收.

### 批量刷新

`MappingArea` 批量 map/unmap/update 时通过 `TlbBatchContext::execute()` 包住循环:

- RISC-V: 单页操作仍刷新本地页, 批处理结束后合并一次全局刷新和 IPI 通知.
- LoongArch64: 当前批处理上下文是本地全量刷新占位实现, 多核 IPI 尚未接入.

### 激活页表

- RISC-V 把根 PPN 写入 `satp` 的 SV39 模式并执行 `sfence.vma`.
- LoongArch64 配置页表 walker CSR, 写 PGDL/PGDH, 设置 ASID, 打开分页并刷新 TLB.高半内核根可由 `set_kernel_root_ppn()` 提供.

## 架构差异

### RISC-V

SV39 PTE 的低 8 位和 `UniversalPTEFlag` 基本同构.`sfence.vma` 用于本地刷新, 多核通过 IPI 请求其他 CPU 刷新.

### LoongArch64

LoongArch PTE 使用 PLV 表达权限级, 使用 NR/NX 表示不可读/不可执行, 因此 flag 转换不是简单位拷贝.页表激活还需要配置 PGDL/PGDH 和 page walk CSR.

## 并发与生命周期约束

- 页表修改必须由上层地址空间锁序列化.页表结构本身不提供内部锁.
- 页表页帧由 `PageTableInner.frames` 持有, 根页和中间页表随 `PageTableInner` 生命周期释放.
- 硬件 TLB 不会自动感知软件页表修改, 所有修改路径必须经过带刷新逻辑的后端方法.
- 通用 `UniversalPTEFlag` 只描述意图, 不应在上层假设某个架构的原始 PTE 位.

## 已知限制

- `PageSize` 仅 `Size4K`.
- RISC-V shootdown 是异步通知, 不等待远端 CPU 确认.
- LoongArch64 批处理刷新和跨核 shootdown 仍未达到 RISC-V 后端同等能力.

## 源码索引

- `os/src/mm/page_table/mod.rs:11` - 活动页表别名和公共类型.
- `os/src/mm/page_table/inner.rs:8` - `PageTableInner` trait.
- `os/src/mm/page_table/page_table_entry.rs:12` - `UniversalPTEFlag`.
- `os/src/mm/page_table/page_table_entry.rs:61` - `PageTableEntry` trait.
- `os/src/arch/riscv/mm/page_table.rs:13` - RISC-V `PageTableInner` 状态.
- `os/src/arch/riscv/mm/page_table.rs:403` - RISC-V 批处理 map/unmap/update.
- `os/src/arch/riscv/mm/page_table.rs:467` - RISC-V `TlbBatchContext`.
- `os/src/arch/riscv/mm/page_table_entry.rs:7` - SV39 PTE flags.
- `os/src/arch/loongarch/mm/page_table.rs:25` - LoongArch64 `PageTableInner` 状态.
- `os/src/arch/loongarch/mm/page_table.rs:399` - LoongArch64 批处理接口.
- `os/src/arch/loongarch/mm/page_table.rs:440` - LoongArch64 `TlbBatchContext`.
- `os/src/arch/loongarch/mm/page_table_entry.rs:35` - LoongArch64 PTE flags.
