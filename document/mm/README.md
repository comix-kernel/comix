# MM 子系统文档

## 简介

MM（Memory Management，内存管理）子系统是 Comix 内核的核心组件，负责管理系统的物理和虚拟内存。该子系统采用清晰的分层架构，将架构无关的抽象层与架构特定的实现层分离，支持 RISC-V 和 LoongArch 等多种硬件平台。

### 主要功能

- **物理内存管理**：物理帧的分配、回收和跟踪
- **虚拟内存管理**：地址空间管理、页表操作、内存映射
- **内核堆分配**：支持动态内存分配的全局分配器
- **架构抽象**：统一的接口支持多种硬件架构

## 模块结构

```
os/src/mm/                          # 架构无关的内存管理层
│
├── mod.rs ........................ MM 子系统初始化入口
│
├── address/ ...................... 地址抽象层
│   ├── address.rs ............... 物理/虚拟地址类型 (Paddr/Vaddr)
│   ├── page_num.rs .............. 物理/虚拟页号类型 (Ppn/Vpn)
│   └── operations.rs ............ 地址运算 trait 定义
│
├── frame_allocator/ .............. 物理帧分配器
│   └── frame_allocator.rs ....... 核心分配器及 RAII 包装器
│
├── global_allocator/ ............. 内核堆分配器
│   ├── global_allocator.rs ...... talc 全局分配器实现
│   └── heap.rs .................. C 风格 kmalloc 接口（未实现）
│
├── page_table/ ................... 页表抽象层
│   ├── page_table.rs ............ PageTableInner trait 定义
│   └── page_table_entry.rs ...... PTE trait 及通用标志位
│
└── memory_space/ ................. 地址空间管理
    ├── memory_space.rs .......... MemorySpace 结构及空间创建
    └── mapping_area.rs .......... MappingArea 映射区域管理

os/src/arch/{riscv,loongarch}/mm/  # 架构特定实现层
│
├── mod.rs ........................ 地址转换函数
├── page_table.rs ................. PageTableInner 实现
└── page_table_entry.rs ........... PageTableEntry 实现
```

## 文档导航

### 核心概念

- **[整体架构](architecture.md)** - MM 子系统的分层设计、模块依赖关系和初始化流程

### 子模块详解

- **[地址抽象层](address.md)** - Paddr/Vaddr/Ppn/Vpn 类型、地址运算和范围操作（**左闭右开区间**）
- **[物理帧分配器](frame_allocator.md)** - 水位线 + 回收栈分配策略、FrameTracker RAII 机制
- **[全局堆分配器](global_allocator.md)** - talc 全局分配器实现和动态内存分配
- **[页表抽象层](page_table.md)** - PageTableInner trait、RISC-V SV39 实现、UniversalPTEFlag

### 地址空间管理

- **[地址空间管理](memory_space.md)** - 地址空间管理、MemorySpace 结构、MappingArea 映射区域及系统调用支持

### API 参考

- **[API 索引](api_reference.md)** - 完整的公共 API 列表及文件位置

## 设计原则

### 1. 架构抽象

MM 子系统使用 trait 系统实现架构抽象，架构特定代码必须实现以下接口：

- `vaddr_to_paddr()` / `paddr_to_vaddr()` - 地址转换函数
- `PageTableInner` trait - 页表操作接口
- `PageTableEntry` trait - 页表项操作接口

### 2. 安全性保障

- **RAII 模式**：物理帧通过 `FrameTracker` 自动管理生命周期
- **类型安全**：物理地址和虚拟地址使用不同类型，防止混用
- **所有权系统**：利用 Rust 的所有权机制防止内存泄漏

### 3. 性能优化

- **帧回收优化**：回收栈自动合并连续帧，减少碎片
- **直接映射**：内核空间使用直接映射，避免页表查找开销
- **对齐分配**：支持对齐的连续帧分配，优化 DMA 等场景

## 重要约定

### Range 语义

所有 range 类型均遵循 **左闭右开区间** 语义：

- `AddressRange::new(start, end)` 表示区间 **[start, end)**
- `PageNumRange::new(start, end)` 表示区间 **[start, end)**
- 迭代器遍历时包含 `start`，不包含 `end`

### 内存布局

Comix 采用**高半核（Higher Half Kernel）**设计，虚拟地址空间分为两个主要区域：

```
虚拟地址空间布局（从高地址到低地址）:

═══════════════════════════════════════════════════════════════════
                        高半核（内核空间）
═══════════════════════════════════════════════════════════════════
0xFFFF_FFFF_FFFF_FFFF
        |
        ... (向高地址扩展的物理内存直接映射区)
        |
[可用物理内存帧]          ← [ekernel, MEMORY_END) 直接映射
[内核堆 Heap]             ← sheap ~ eheap (16MB，talc 分配器)
[内核 BSS 段 .bss]        ← sbss ~ ebss
[内核数据段 .data]        ← sdata ~ edata
[内核只读数据段 .rodata]  ← srodata ~ erodata
[内核代码段 .text]        ← stext ~ etext
        |
0xFFFF_FFC0_8020_0000 ← VIRTUAL_BASE (内核加载地址)
        |
地址上半底部 ← VADDR_START (内核空间基址)

物理地址映射: vaddr = paddr | 0xFFFF_FFC0_0000_0000

═══════════════════════════════════════════════════════════════════
                         低半核（用户空间）
═══════════════════════════════════════════════════════════════════
地址下半顶部
        |
[USER_STACK]              ← 用户栈区域 (4MB)
        |
        ... (动态扩展空间)
        |
[USER_HEAP]               ← 用户堆区域（可动态扩展，最大 64MB）
[USER_DATA]               ← 用户数据段 (.data, .bss)
[USER_TEXT]               ← 用户代码段 (.text)
        |
0x0000_0000_0000_0000
```

**关键地址常量**：

*内核空间*：
- `VADDR_START = 0xFFFF_FFC0_0000_0000` - 内核虚拟地址空间基址（RISC-V）
- `VIRTUAL_BASE = 0xFFFF_FFC0_8020_0000` - 内核实际加载地址
- `PHYSICAL_BASE = 0x8020_0000` - 内核物理加载地址
- `MEMORY_END = 0x8800_0000` - 物理内存结束地址（128MB，QEMU virt）

*用户空间*：
- `USER_STACK_TOP - 用户栈顶

**地址转换规则**（RISC-V SV39）：
- 物理地址 → 虚拟地址：`vaddr = paddr | VADDR_START`
- 虚拟地址 → 物理地址：`paddr = vaddr & 0x0000_003F_FFFF_FFFF`

## 快速开始

### 初始化流程

MM 子系统在 `mm::init()` 中按以下顺序初始化：

1. **物理帧分配器初始化** - 管理 `[ekernel, MEMORY_END)` 物理内存区域
2. **内核堆分配器初始化** - 初始化全局堆分配器
3. **内核地址空间创建** - 创建并激活内核页表

```rust
// os/src/mm/mod.rs:33
pub fn init() {
    // 1. 初始化物理帧分配器
    let ekernel_paddr = unsafe { vaddr_to_paddr(ekernel as usize) };
    init_frame_allocator(Ppn::from_addr_ceil(ekernel_paddr),
                         Ppn::from_addr_floor(MEMORY_END));

    // 2. 初始化堆分配器
    init_heap();

    // 3. 创建并激活内核地址空间
    #[cfg(target_arch = "riscv64")] {
        let root_ppn = with_kernel_space(|space| space.root_ppn());
        crate::arch::mm::PageTableInner::activate(root_ppn);
    }
}
```

### 常见操作示例

#### 分配物理帧

```rust
use crate::mm::frame_allocator::alloc_frame;

// 分配单个物理帧（自动释放）
let frame = alloc_frame().expect("Failed to allocate frame");
let ppn = frame.ppn();
```

#### 创建地址映射

```rust
use crate::mm::memory_space::MemorySpace;

// 创建用户地址空间
let mut space = MemorySpace::from_elf(elf_data);

// 映射匿名内存
space.mmap(start_vaddr, len, prot);
```

#### 地址转换

```rust
use crate::mm::address::{Vaddr, Paddr};

// 虚拟地址转物理地址
let vaddr = Vaddr::new(0xffff_ffc0_8000_0000);
let paddr = vaddr.to_paddr();

// 物理地址转虚拟地址
let vaddr_back = paddr.to_vaddr();
```

## 相关资源

- **源代码位置**：`os/src/mm/` 和 `os/src/arch/{riscv,loongarch}/mm/`
- **配置常量**：`os/src/config.rs`

## 版本信息

- **Rust 版本**：nightly-2025-01-13
- **支持架构**：RISC-V (SV39), LoongArch (TODO)
- **页面大小**：4KB（大页支持已暂时禁用）
