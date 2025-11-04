# 页表抽象层

## 概述

页表抽象层提供架构无关的页表操作接口，通过 trait 系统将通用逻辑与硬件特定实现分离。目前已实现 RISC-V SV39 三级页表。

### 设计目标

1. **架构抽象**：统一的 trait 接口支持多种架构
2. **类型安全**：通过 Rust 类型系统防止错误操作
3. **灵活标志位**：UniversalPTEFlag 屏蔽架构差异
4. **易于扩展**：新增架构只需实现 trait

## 核心接口

### PageTableInner Trait

页表的核心操作接口：

```rust
pub trait PageTableInner<T: PageTableEntry> {
    // 架构常量
    const LEVELS: usize;          // 页表级数（SV39 为 3）
    const MAX_VA_BITS: usize;     // 虚拟地址位宽（SV39 为 39）
    const MAX_PA_BITS: usize;     // 物理地址位宽（SV39 为 56）

    // 生命周期管理
    fn new() -> Self;
    fn from_ppn(root_ppn: Ppn) -> Self;
    fn activate(ppn: Ppn);

    // TLB 管理
    fn tlb_flush(vpn: Vpn);
    fn tlb_flush_all();

    // 核心操作
    fn map(&mut self, vpn: Vpn, ppn: Ppn, page_size: PageSize,
           flags: UniversalPTEFlag) -> PagingResult<()>;
    fn unmap(&mut self, vpn: Vpn) -> PagingResult<()>;
    fn translate(&self, vaddr: Vaddr) -> Option<Paddr>;

    // 查询操作
    fn walk(&self, vpn: Vpn) -> PagingResult<(Ppn, PageSize, UniversalPTEFlag)>;
    fn root_ppn(&self) -> Ppn;
}
```

### UniversalPTEFlag

架构无关的页表项标志位：

```rust
pub struct UniversalPTEFlag(u8);

impl UniversalPTEFlag {
    // 基础标志
    pub const VALID: Self = Self(1 << 0);       // 页表项有效
    pub const READABLE: Self = Self(1 << 1);    // 可读
    pub const WRITABLE: Self = Self(1 << 2);    // 可写
    pub const EXECUTABLE: Self = Self(1 << 3);  // 可执行
    pub const USER_ACCESSIBLE: Self = Self(1 << 4);  // 用户态可访问
    pub const GLOBAL: Self = Self(1 << 5);      // 全局映射
    pub const ACCESSED: Self = Self(1 << 6);    // 已访问
    pub const DIRTY: Self = Self(1 << 7);       // 已修改

    // 组合标志
    pub fn kernel_r() -> Self {
        Self::VALID | Self::READABLE
    }

    pub fn kernel_rw() -> Self {
        Self::VALID | Self::READABLE | Self::WRITABLE
    }

    pub fn kernel_rx() -> Self {
        Self::VALID | Self::READABLE | Self::EXECUTABLE
    }

    pub fn user_r() -> Self {
        Self::VALID | Self::READABLE | Self::USER_ACCESSIBLE
    }

    pub fn user_rw() -> Self {
        Self::VALID | Self::READABLE | Self::WRITABLE | Self::USER_ACCESSIBLE
    }

    pub fn user_rwx() -> Self {
        Self::VALID | Self::READABLE | Self::WRITABLE |
        Self::EXECUTABLE | Self::USER_ACCESSIBLE
    }
}
```

## RISC-V SV39 实现

### SV39 页表结构

```
39 位虚拟地址:
┌─────────┬──────────┬──────────┬──────────┬────────────┐
│ 63...39 │  38...30 │  29...21 │  20...12 │   11...0   │
│ (符号位) │  VPN[2]  │  VPN[1]  │  VPN[0]  │   offset   │
└─────────┴──────────┴──────────┴──────────┴────────────┘
    25 位      9 位       9 位       9 位        12 位

56 位物理地址:
┌─────────┬──────────────────────────────┬────────────┐
│ 55...44 │         43...12              │   11...0   │
│ (保留)  │          PPN                 │   offset   │
└─────────┴──────────────────────────────┴────────────┘
   12 位             32 位                   12 位

页表项 (PTE):
┌────────┬──────────────────────┬───────┬─┬─┬─┬─┬─┬─┬─┬─┐
│ 63...54│      53...10         │ 9...8 │D│A│G│U│X│W│R│V│
│  (保留) │        PPN           │  RSW  │ │ │ │ │ │ │ │ │
└────────┴──────────────────────┴───────┴─┴─┴─┴─┴─┴─┴─┴─┘
  10 位          44 位              2 位   标志位（8位）
```

### 地址转换

Comix 使用**直接映射**方式：

```rust
// 物理地址 → 虚拟地址
pub const fn paddr_to_vaddr(paddr: usize) -> usize {
    paddr | 0xffff_ffc0_0000_0000
}

// 虚拟地址 → 物理地址
pub const unsafe fn vaddr_to_paddr(vaddr: usize) -> usize {
    vaddr & 0x0000_003f_ffff_ffff
}
```

### 页表遍历

```
三级页表查找流程:

Virtual Address: VPN[2] | VPN[1] | VPN[0] | offset
                   ↓
┌──────────────────────────────────────┐
│  Level 2 (Root Page Table)           │
│  Entry[VPN[2]] → PPN of Level 1      │──┐
└──────────────────────────────────────┘  │
                                           ↓
┌──────────────────────────────────────┐
│  Level 1 Page Table                  │
│  Entry[VPN[1]] → PPN of Level 0      │──┐
└──────────────────────────────────────┘  │
                                           ↓
┌──────────────────────────────────────┐
│  Level 0 Page Table (Leaf)           │
│  Entry[VPN[0]] → PPN of Data Page    │──┐
└──────────────────────────────────────┘  │
                                           ↓
                                 Physical Page + offset
```

### TLB 管理

```rust
// 刷新单个页
pub fn tlb_flush(vpn: Vpn) {
    unsafe {
        asm!("sfence.vma {0}, zero", in(reg) vpn.as_usize());
    }
}

// 刷新所有页
pub fn tlb_flush_all() {
    unsafe {
        asm!("sfence.vma");
    }
}
```

## 基本使用

### 创建页表

```rust
// 创建新页表（自动分配根页表帧）
let mut page_table = ActivePageTableInner::new();

// 从已有根页号创建
let page_table = PageTableInner::from_ppn(root_ppn);
```

### 映射页面

```rust
// 映射单个 4K 页
let vpn = Vpn::new(0x1000);
let ppn = Ppn::new(0x8000_1);
page_table.map(
    vpn,
    ppn,
    PageSize::Size4K,
    UniversalPTEFlag::user_rw()
)?;

// TLB 刷新
ActivePageTableInner::tlb_flush(vpn);
```

### 取消映射

```rust
page_table.unmap(vpn)?;
ActivePageTableInner::tlb_flush(vpn);
```

### 地址翻译

```rust
let vaddr = Vaddr::new(0x1000_0000);
if let Some(paddr) = page_table.translate(vaddr) {
    println!("VA {:#x} → PA {:#x}", vaddr.as_usize(), paddr.as_usize());
} else {
    println!("Page fault: unmapped address");
}
```

### 查询映射信息

```rust
match page_table.walk(vpn) {
    Ok((ppn, size, flags)) => {
        println!("Mapped: VPN {:#x} → PPN {:#x}", vpn.as_usize(), ppn.as_usize());
        println!("Page size: {:?}", size);
        println!("Flags: {:?}", flags);
    }
    Err(e) => println!("Walk failed: {:?}", e),
}
```

## 使用场景

### 场景 1：内核地址空间创建

```rust
pub fn new_kernel() -> Self {
    let mut space = MemorySpace::new();

    // 映射跳板页
    space.map_trampoline();

    // 映射内核段
    space.push(MappingArea::new(
        VaddrRange::new(Vaddr::new(stext as usize), Vaddr::new(etext as usize)),
        MapType::Direct,
        UniversalPTEFlag::kernel_rx(),
        AreaType::KernelText,
    ));

    // 映射内核数据段
    space.push(MappingArea::new(
        VaddrRange::new(Vaddr::new(sdata as usize), Vaddr::new(edata as usize)),
        MapType::Direct,
        UniversalPTEFlag::kernel_rw(),
        AreaType::KernelData,
    ));

    // 直接映射物理内存
    let phys_mem_end = paddr_to_vaddr(MEMORY_END);
    space.push(MappingArea::new(
        VaddrRange::new(Vaddr::new(ekernel as usize), Vaddr::new(phys_mem_end)),
        MapType::Direct,
        UniversalPTEFlag::kernel_rw(),
        AreaType::PhysicalMemory,
    ));

    space
}
```

### 场景 2：用户程序加载

```rust
pub fn from_elf(elf_data: &[u8]) -> Result<Self, ElfError> {
    let elf = xmas_elf::ElfFile::new(elf_data)?;
    let mut space = MemorySpace::new();

    for ph in elf.program_iter() {
        if ph.get_type() != ProgramHeaderType::Load {
            continue;
        }

        let start_va = ph.virtual_addr() as usize;
        let end_va = (ph.virtual_addr() + ph.mem_size()) as usize;
        let flags = ph_flags_to_universal(ph.flags());

        // 创建映射区域
        let area = MappingArea::new(
            VaddrRange::new(Vaddr::new(start_va), Vaddr::new(end_va)),
            MapType::Framed,  // 为每页分配物理帧
            flags,
            AreaType::UserData,
        );

        space.push(area);
    }

    Ok(space)
}
```

## 错误处理

```rust
#[derive(Debug)]
pub enum PagingError {
    PageFault,        // 页面不存在
    AlreadyMapped,    // 页面已映射
    InvalidFlags,     // 标志位无效
    FrameAllocFailed, // 物理帧分配失败
}

pub type PagingResult<T> = Result<T, PagingError>;
```

## 常见问题

### Q1: 为什么需要刷新 TLB？

**A**: TLB 缓存虚拟地址到物理地址的翻译结果。修改页表后，必须刷新 TLB 以保证硬件使用最新映射。

### Q2: 什么时候使用 tlb_flush_all？

**A**:
- 切换页表（如进程切换）
- 批量修改映射时
- 单个 `tlb_flush` 适用于修改少量页面

### Q3: translate 和 walk 的区别？

**A**:
- `translate(vaddr)`：快速翻译，仅返回物理地址
- `walk(vpn)`：返回完整映射信息（PPN、大小、标志），用于调试

### Q4: 为什么 VADDR_START 是 0xffff_ffc0_0000_0000？

**A**: 这是 SV39 高半核的起始地址：
- Bit 38 = 1，符号扩展后 bits [63:39] 全为 1
- 低 38 位全为 0
- 结果：`0xffff_ffc0_0000_0000`

## 性能考量

### TLB 性能

- TLB 命中率通常 > 95%
- TLB Miss 惩罚：~100 CPU 周期
- 合理规划映射可提高 TLB 命中率

### 大页支持

当前实现中大页已暂时禁用。未来启用时：
- 2MB 大页：减少 TLB 压力
- 1GB 巨页：适用于大块连续内存

## 相关文档

- [地址抽象层](address.md) - Vpn/Ppn 类型
- [物理帧分配器](frame_allocator.md) - 页表帧的分配
- [整体架构](architecture.md) - MM 子系统分层设计
- [API 参考](api_reference.md) - 完整 API 列表

## 参考实现

- **架构无关层**：`os/src/mm/page_table/`
- **RISC-V 实现**：`os/src/arch/riscv/mm/page_table.rs`
- **RISC-V 规范**：SV39 Paging Scheme
