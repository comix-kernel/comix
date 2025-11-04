# MM 子系统整体架构

## 概述

Comix 内核的 MM（Memory Management）子系统采用分层架构设计，将架构无关的通用抽象与架构特定的实现清晰分离。这种设计使得内核能够在不修改核心逻辑的情况下支持多种硬件架构（RISC-V、LoongArch 等）。

## 分层架构

### 架构层次图

```
┌─────────────────────────────────────────────────────────────┐
│                      应用层                                  │
│            (系统调用: mmap/munmap/brk 等)                     │
└─────────────────────────┬───────────────────────────────────┘
                          │
┌─────────────────────────┴───────────────────────────────────┐
│            架构无关层 (os/src/mm/)                            │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  memory_space/  - MemorySpace 地址空间管理           │   │
│  │                 - MappingArea 映射区域管理            │   │
│  └────────────────┬─────────────────┬───────────────────┘   │
│                   │                 │                        │
│  ┌────────────────▼─────────────┐  ┌▼──────────────────┐   │
│  │  page_table/                 │  │ frame_allocator/  │   │
│  │  - PageTableInner trait      │  │ - FrameAllocator  │   │
│  │  - PageTableEntry trait      │  │ - FrameTracker    │   │
│  │  - UniversalPTEFlag          │  └───────────────────┘   │
│  └──────────────┬───────────────┘                           │
│                 │                 ┌──────────────────────┐  │
│  ┌──────────────▼──────────────┐  │ global_allocator/   │  │
│  │  address/                   │  │ - talc Allocator    │  │
│  │  - Paddr/Vaddr              │  └──────────────────────┘  │
│  │  - Ppn/Vpn                  │                             │
│  │  - 地址运算 trait            │                             │
│  └─────────────────────────────┘                             │
└─────────────────────────┬───────────────────────────────────┘
                          │
┌─────────────────────────┴───────────────────────────────────┐
│         架构特定层 (os/src/arch/{riscv,loongarch}/mm/)       │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  - vaddr_to_paddr() / paddr_to_vaddr()               │   │
│  │  - PageTableInner 实现 (如 RISC-V SV39)               │   │
│  │  - PageTableEntry 实现 (如 SV39 PTE 格式)             │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────┬───────────────────────────────────┘
                          │
┌─────────────────────────┴───────────────────────────────────┐
│                      硬件层                                  │
│       (MMU, TLB, 物理内存, SATP/PGDH 寄存器等)               │
└─────────────────────────────────────────────────────────────┘
```

### 各层职责

#### 1. 架构无关层 (os/src/mm/)

提供通用的内存管理抽象，不包含任何架构特定代码。

**职责**：
- 定义统一的地址类型和页号类型
- 提供物理帧分配和回收算法
- 实现地址空间管理和映射区域管理
- 定义页表操作的 trait 接口
- 管理内核堆分配器

**关键设计**：
- 使用 trait 定义架构无关接口
- 通过条件编译引用架构特定实现
- 所有公共 API 均在此层暴露

#### 2. 架构特定层 (os/src/arch/{riscv,loongarch}/mm/)

为特定硬件架构提供具体实现。

**职责**：
- 实现虚拟地址与物理地址的转换逻辑
- 实现 PageTableInner trait（页表遍历、映射、TLB 管理等）
- 实现 PageTableEntry trait（PTE 标志位操作）
- 提供架构特定的常量和配置

**当前支持架构**：
- **RISC-V**：完整实现 SV39 三级页表
- **LoongArch**：TODO（占位符已预留）

## 模块依赖关系

```
                        ┌──────────────┐
                        │   mm/mod.rs  │
                        │  (初始化器)   │
                        └───────┬──────┘
                                │ init()
                ┌───────────────┼───────────────┐
                │               │               │
                ▼               ▼               ▼
        ┌───────────┐   ┌──────────┐   ┌──────────────┐
        │frame_     │   │ global_  │   │memory_space  │
        │allocator  │   │allocator │   │(内核空间)     │
        └───────────┘   └──────────┘   └──────┬───────┘
                │                              │
                │         ┌────────────────────┘
                │         │
                ▼         ▼
        ┌─────────────────────────┐
        │    page_table/          │
        │  (PageTableInner trait) │
        └────────────┬────────────┘
                     │
                     ▼
        ┌─────────────────────────┐
        │    address/             │
        │  (Paddr/Vaddr/Ppn/Vpn)  │
        └────────────┬────────────┘
                     │
                     ▼
        ┌─────────────────────────┐
        │   arch/*/mm/            │
        │  - vaddr_to_paddr()     │
        │  - paddr_to_vaddr()     │
        │  - PageTable 实现        │
        └─────────────────────────┘
```

### 关键依赖路径

1. **内存分配路径**：
   ```
   memory_space → mapping_area → frame_allocator → FrameTracker
   ```

2. **地址转换路径**：
   ```
   Vaddr/Paddr → arch::mm::{vaddr_to_paddr, paddr_to_vaddr}
   ```

3. **页表操作路径**：
   ```
   MemorySpace → PageTableInner trait → arch::mm::PageTableInner
   ```

4. **初始化路径**：
   ```
   mm::init() → frame_allocator::init() → global_allocator::init() → MemorySpace::new_kernel()
   ```

## 初始化流程

### 启动序列

```
┌────────────────────────────────────────────────────────────┐
│  1. 内核入口 (rust_main)                                    │
└─────────────────────┬──────────────────────────────────────┘
                      │
                      ▼
┌────────────────────────────────────────────────────────────┐
│  2. mm::init()                                              │
│     ├─ 获取可用物理内存范围 [ekernel, MEMORY_END)          │
│     ├─ 初始化物理帧分配器                                   │
│     ├─ 初始化内核堆分配器 (talc)                            │
│     └─ 创建并激活内核地址空间                               │
└─────────────────────┬──────────────────────────────────────┘
                      │
      ┌───────────────┼───────────────┐
      │               │               │
      ▼               ▼               ▼
┌──────────┐  ┌──────────────┐  ┌──────────────────┐
│ 物理帧   │  │  内核堆      │  │  内核页表        │
│ 分配器   │  │  分配器      │  │  创建与激活      │
│ 就绪     │  │  就绪        │  │  就绪            │
└──────────┘  └──────────────┘  └──────────────────┘
```

### 详细步骤

#### 第一步：物理帧分配器初始化

```rust
// os/src/mm/mod.rs:36-41
let ekernel_paddr = unsafe { vaddr_to_paddr(ekernel as usize) };
let start = Ppn::from_addr_ceil(Paddr::new(ekernel_paddr));
let end = Ppn::from_addr_floor(Paddr::new(MEMORY_END));
init_frame_allocator(start, end);
```

**作用**：
- 计算内核结束地址到物理内存结束的可用区域
- 初始化全局帧分配器 `FRAME_ALLOCATOR`
- 此时可以开始分配物理帧

#### 第二步：内核堆分配器初始化

```rust
// os/src/mm/mod.rs:44
init_heap();
```

**作用**：
- 初始化 talc 全局堆分配器
- 注册内核堆区域 `[sheap, eheap)`（由链接脚本定义）
- 此时可以使用 `alloc` crate 进行动态内存分配（Vec、Box 等）

#### 第三步：内核地址空间创建

```rust
// os/src/mm/mod.rs:47-51
#[cfg(target_arch = "riscv64")] {
    let root_ppn = with_kernel_space(|space| space.root_ppn());
    crate::arch::mm::PageTableInner::activate(root_ppn);
}
```

**作用**：
- 调用 `MemorySpace::new_kernel()` 创建内核地址空间
- 映射内核各段（text/rodata/data/bss/heap）
- 直接映射所有物理内存到高半核
- 写入 SATP 寄存器并刷新 TLB，启用分页

### 内核地址空间构建细节

```
MemorySpace::new_kernel() 执行以下映射:

1. 跳板页 (Trampoline)
   [usize::MAX-PAGE_SIZE+1, usize::MAX+1) → 跳板代码物理页

2. 内核代码段 (.text)
   [stext, etext) → 对应物理地址，权限: R+X

3. 内核只读数据段 (.rodata)
   [srodata, erodata) → 对应物理地址，权限: R

4. 内核数据段 (.data)
   [sdata, edata) → 对应物理地址，权限: R+W

5. 内核栈段 (.bss.stack)
   [boot_stack, boot_stack_top) → 对应物理地址，权限: R+W

6. 内核 BSS 段 (.bss)
   [sbss, ebss) → 对应物理地址，权限: R+W

7. 内核堆段
   [sheap, eheap) → 对应物理地址，权限: R+W

8. 直接映射物理内存
   [ekernel, MEMORY_END) → 对应物理地址，权限: R+W
   (用于访问用户进程的物理页面)
```

## 架构抽象模式

### 1. 条件编译导出

通过 `#[cfg(target_arch = "...")]` 实现架构选择：

```rust
// os/src/arch/mod.rs:6-10
#[cfg(target_arch = "loongarch64")]
pub use self::loongarch::*;

#[cfg(target_arch = "riscv64")]
pub use riscv::*;
```

### 2. Trait 接口契约

架构特定代码必须实现以下 trait：

```rust
// PageTableInner trait (os/src/mm/page_table/page_table.rs:5-52)
pub trait PageTableInner<T: PageTableEntry> {
    const LEVELS: usize;          // 页表级数（如 SV39 为 3）
    const MAX_VA_BITS: usize;     // 虚拟地址位宽（如 SV39 为 39）
    const MAX_PA_BITS: usize;     // 物理地址位宽（如 SV39 为 56）

    // TLB 管理
    fn tlb_flush(vpn: Vpn);
    fn tlb_flush_all();

    // 生命周期
    fn new() -> Self;
    fn from_ppn(ppn: Ppn) -> Self;
    fn activate(ppn: Ppn);

    // 核心操作
    fn map(&mut self, vpn: Vpn, ppn: Ppn, page_size: PageSize,
           flags: UniversalPTEFlag) -> PagingResult<()>;
    fn unmap(&mut self, vpn: Vpn) -> PagingResult<()>;
    fn translate(&self, vaddr: Vaddr) -> Option<Paddr>;
    // ...更多方法
}
```

### 3. 通用标志位转换

通过 `UniversalPTEFlag` 实现架构无关的权限表示：

```rust
// os/src/mm/page_table/page_table_entry.rs:4-11
pub struct UniversalPTEFlag(u8);

impl UniversalPTEFlag {
    // 低 8 位兼容 RISC-V SV39 格式
    pub const V: Self = Self(1 << 0);  // Valid
    pub const R: Self = Self(1 << 1);  // Readable
    pub const W: Self = Self(1 << 2);  // Writable
    pub const X: Self = Self(1 << 3);  // Executable
    // ...
}
```

架构特定的 PTE 标志位通过 `UniversalConvertableFlag` trait 转换：

```rust
// os/src/arch/riscv/mm/page_table_entry.rs:142-146
impl UniversalConvertableFlag for SV39PTEFlags {
    fn from_universal(flag: UniversalPTEFlag) -> Self {
        Self::from_bits(flag.bits() & 0xff).unwrap()
    }
}
```

### 4. 地址转换函数

每个架构必须提供地址转换函数：

```rust
// RISC-V 实现 (os/src/arch/riscv/mm/mod.rs:8-15)
pub const VADDR_START: usize = 0xffff_ffc0_0000_0000;
pub const PADDR_MASK: usize = 0x0000_003f_ffff_ffff;

pub const unsafe fn vaddr_to_paddr(vaddr: usize) -> usize {
    vaddr & PADDR_MASK  // 提取低 38 位
}

pub const fn paddr_to_vaddr(paddr: usize) -> usize {
    paddr | VADDR_START  // 添加高半核前缀
}
```

## 关键设计决策

### 1. 为什么使用直接映射？

**内核空间的直接映射设计**（物理地址 → 物理地址 + VADDR_START）：

**优势**：
- 访问物理内存无需页表查找，性能优秀
- 简化内核代码，地址转换仅需位运算
- 便于访问用户进程的物理页面（用于拷贝数据）

**代价**：
- 需要占用较大的虚拟地址空间（高半核）
- 仅适用于 64 位架构

### 2. 为什么分离 Paddr 和 Vaddr？

**类型安全设计**：

```rust
#[repr(transparent)]
pub struct Paddr(usize);

#[repr(transparent)]
pub struct Vaddr(usize);
```

**优势**：
- 编译期防止物理地址和虚拟地址混用
- 明确表达函数参数的地址类型语义
- 通过 `repr(transparent)` 保证零成本抽象

### 3. 为什么使用 RAII 管理物理帧？

**FrameTracker 设计**：

```rust
// os/src/mm/frame_allocator/frame_allocator.rs:24-35
pub struct FrameTracker {
    ppn: Ppn,
}

impl Drop for FrameTracker {
    fn drop(&mut self) {
        dealloc_frame(self.ppn);
    }
}
```

**优势**：
- 自动释放，防止内存泄漏
- 配合 Rust 所有权系统，编译期检查
- 支持通过 `clone()` 显式拷贝，避免意外共享

### 4. 为什么暂时禁用大页？

**当前限制**：
- `PageSize::Size2M` 和 `PageSize::Size1G` 枚举存在但未启用
- 映射区域的 extend/shrink 仅支持 4K 页

**原因**：
- 大页扩展/收缩逻辑复杂，需要拆分/合并页表项
- 帧分配器需要支持对齐的大块分配
- 需要更多测试验证正确性

**未来计划**：
- 代码中已预留大页相关逻辑（已注释）
- 完善后可启用，优化 TLB 性能

### 5. 为什么使用左闭右开区间？

**Range 语义统一**：

```rust
// AddressRange/PageNumRange 均为 [start, end)
let range = VpnRange::new(start_vpn, end_vpn);
// 包含 start_vpn，不包含 end_vpn
```

**优势**：
- 符合 Rust 标准库惯例（`a..b` 即 `[a, b)`）
- 便于计算长度：`len = end - start`
- 避免边界处理歧义

## 架构扩展指南

### 添加新架构支持

**步骤**：

1. **创建架构目录**：
   ```
   os/src/arch/{新架构}/mm/
   ├── mod.rs
   ├── page_table.rs
   └── page_table_entry.rs
   ```

2. **实现地址转换函数**（`mod.rs`）：
   ```rust
   pub const unsafe fn vaddr_to_paddr(vaddr: usize) -> usize;
   pub const fn paddr_to_vaddr(paddr: usize) -> usize;
   ```

3. **实现 PageTableEntry trait**（`page_table_entry.rs`）：
   - 定义 PTE 结构体（如 `struct RV64PTE(u64)`）
   - 实现 `PageTableEntry` trait 的所有方法
   - 实现 `UniversalConvertableFlag` 转换

4. **实现 PageTableInner trait**（`page_table.rs`）：
   - 定义页表结构体（如 `struct PageTableInner`）
   - 实现所有必需方法（map/unmap/translate/walk 等）
   - 实现 TLB 管理和页表激活

5. **更新架构选择器**（`os/src/arch/mod.rs`）：
   ```rust
   #[cfg(target_arch = "新架构")]
   pub use self::新架构::*;
   ```

6. **添加测试**：
   - 单元测试验证地址转换正确性
   - 集成测试验证页表操作

### 注意事项

- 确保 `MAX_VA_BITS` 和 `MAX_PA_BITS` 常量正确
- TLB 刷新操作必须正确实现（错误可能导致诡异 bug）
- 大页支持可选，但需在 `is_huge()` 中正确检测
- 参考 RISC-V 实现（`os/src/arch/riscv/mm/`）作为范例

## 性能考量

### 关键优化

1. **帧分配器回收优化**：
   - 回收时自动合并栈顶连续帧
   - 减少碎片，提高分配连续帧的成功率

2. **直接映射避免 TLB miss**：
   - 内核访问物理内存时无需查页表
   - 减少 TLB 压力

3. **BTreeMap 存储帧映射**：
   - `MappingArea` 使用 `BTreeMap<Vpn, TrackedFrames>`
   - O(log n) 查找性能，支持范围查询

4. **零拷贝地址转换**：
   - `repr(transparent)` 确保地址类型无运行时开销
   - 地址转换函数标记为 `const` 和 `inline`

### 潜在瓶颈

- **全局锁**：`FRAME_ALLOCATOR` 使用 `Mutex` 保护，高并发时可能成为瓶颈
- **TLB 刷新**：频繁的 `unmap` 操作导致 TLB 失效
- **页表遍历**：三级页表查找需要 3 次内存访问（可通过 TLB 缓存缓解）

## 安全性

### 关键安全机制

1. **类型系统防护**：
   - 物理地址和虚拟地址类型隔离
   - 页表项权限通过 `UniversalPTEFlag` 显式指定

2. **RAII 资源管理**：
   - `FrameTracker` 自动释放物理帧
   - `MappingArea` 在 Drop 时自动取消映射

3. **所有权检查**：
   - 页表所有权明确（`MemorySpace` 拥有页表）
   - 帧所有权通过 `FrameTracker` 转移

4. **保护页机制**：
   - 用户栈和 trap 上下文间插入未映射页
   - 栈溢出时触发缺页异常而非静默覆盖

### 已知限制

- **Unsafe 代码**：地址转换函数标记为 `unsafe`，调用者需保证地址有效性
- **直接映射风险**：内核可直接访问所有物理内存，需小心处理指针
- **未实现 ASLR**：地址空间布局固定，存在安全隐患

## 参考资料

- **RISC-V 特权架构规范**：SV39 页表格式定义
- **LoongArch 架构手册**：待实现架构的参考
- **Rust 嵌入式书**：裸机编程最佳实践
- **xv6-riscv**：经典教学操作系统，内存管理参考

## 总结

Comix 的 MM 子系统通过清晰的分层架构和 trait 抽象，实现了高度模块化和可扩展的设计。架构无关层提供统一接口，架构特定层提供硬件适配，二者通过 trait 系统和条件编译无缝集成。该设计既保证了代码的可维护性，又为未来扩展（如 LoongArch 支持、大页功能）奠定了坚实基础。
