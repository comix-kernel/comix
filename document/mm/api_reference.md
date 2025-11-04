# MM 子系统 API 参考手册

## 概述

本文档提供 MM 子系统所有公共 API 的快速参考，按模块分类并标注源代码位置。

## 初始化 API

### mm::init()

```rust
pub fn init()
```

**功能**：初始化整个 MM 子系统

**调用顺序**：
1. 初始化物理帧分配器
2. 初始化内核堆分配器
3. 创建并激活内核地址空间

**源代码**：`os/src/mm/mod.rs:33`

**示例**：
```rust
fn rust_main() {
    mm::init();  // 第一个调用
    // 此后可以使用所有内存管理功能
}
```

---

## 地址抽象层 API (address/)

### Paddr / Vaddr

#### 创建

```rust
impl Paddr {
    pub fn new(value: usize) -> Self
    pub fn as_usize(&self) -> usize
}
```

**源代码**：`os/src/mm/address/address.rs:20-53`

#### 转换

```rust
impl Paddr {
    pub fn to_vaddr(self) -> Vaddr                    // 物理→虚拟
}

impl Vaddr {
    pub fn to_paddr(self) -> Paddr                    // 虚拟→物理（unsafe）
}
```

#### 对齐

```rust
impl AlignOps for Paddr/Vaddr {
    fn is_aligned(&self, align: usize) -> bool
    fn align_up(&self, align: usize) -> Self
    fn align_down(&self, align: usize) -> Self
    fn is_page_aligned(&self) -> bool
    fn align_up_to_page(&self) -> Self
    fn align_down_to_page(&self) -> Self
}
```

**源代码**：`os/src/mm/address/operations.rs:44-75`

### Ppn / Vpn

#### 创建

```rust
impl Ppn/Vpn {
    pub fn new(value: usize) -> Self
    pub fn from_addr_floor(addr: Paddr/Vaddr) -> Self  // 向下取整
    pub fn from_addr_ceil(addr: Paddr/Vaddr) -> Self   // 向上取整
}
```

**源代码**：`os/src/mm/address/page_num.rs:7-64`

#### 地址转换

```rust
impl PageNum for Ppn/Vpn {
    fn start_addr(self) -> Paddr/Vaddr    // 页起始地址
    fn end_addr(self) -> Paddr/Vaddr      // 页结束地址（下一页起始）
    fn step(self) -> Self                 // 前进一页
    fn step_back(self) -> Self            // 后退一页
    fn offset(self, offset: isize) -> Self // 偏移多页
}
```

### AddressRange / PageNumRange

#### 创建

```rust
impl<T> AddressRange<T>/PageNumRange<T> {
    pub fn new(start: T, end: T) -> Self  // [start, end) 左闭右开
    pub fn start(&self) -> T
    pub fn end(&self) -> T
    pub fn len(&self) -> usize
    pub fn is_empty(&self) -> bool
}
```

**源代码**：`os/src/mm/address/address.rs:141-233`、`os/src/mm/address/page_num.rs:93-185`

#### 区间运算

```rust
pub fn contains(&self, item: &T) -> bool
pub fn intersects(&self, other: &Self) -> bool
pub fn intersection(&self, other: &Self) -> Option<Self>
pub fn union(&self, other: &Self) -> Option<Self>
```

**重要**：所有 Range 类型均为**左闭右开区间 [start, end)**

---

## 物理帧分配器 API (frame_allocator/)

### 分配

```rust
pub fn alloc_frame() -> FrameAllocResult<FrameTracker>
pub fn alloc_frames(n: usize) -> FrameAllocResult<Vec<FrameTracker>>
pub fn alloc_contig_frames(n: usize) -> FrameAllocResult<FrameRangeTracker>
pub fn alloc_contig_frames_aligned(n: usize, align: usize) -> FrameAllocResult<FrameRangeTracker>
```

**源代码**：`os/src/mm/frame_allocator/frame_allocator.rs:219-236`

**示例**：
```rust
// 单帧
let frame = alloc_frame()?;

// 10个非连续帧
let frames = alloc_frames(10)?;

// 256个连续帧（1MB）
let contig = alloc_contig_frames(256)?;

// 512个连续帧，2MB对齐
let aligned = alloc_contig_frames_aligned(512, 512)?;
```

### FrameTracker

```rust
impl FrameTracker {
    pub fn ppn(&self) -> Ppn
    pub fn start_paddr(&self) -> Paddr
    pub fn as_slice<T>(&self) -> &[T]
    pub fn as_slice_mut<T>(&mut self) -> &mut [T]
}
```

**源代码**：`os/src/mm/frame_allocator/frame_allocator.rs:24-71`

**RAII**：自动释放（`Drop`）

### FrameRangeTracker

```rust
impl FrameRangeTracker {
    pub fn start_ppn(&self) -> Ppn
    pub fn end_ppn(&self) -> Ppn
    pub fn start_paddr(&self) -> Paddr
    pub fn end_paddr(&self) -> Paddr
    pub fn iter(&self) -> impl Iterator<Item = Ppn>
}
```

**源代码**：`os/src/mm/frame_allocator/frame_allocator.rs:74-132`

### 错误类型

```rust
pub enum FrameAllocError {
    OutOfMemory,
    InvalidAddress,
    AlignmentError,
}
```

---

## 内核堆分配器 API (global_allocator/)

### 初始化

```rust
pub fn init_heap()
```

**功能**：初始化全局堆分配器（16 MB）

**源代码**：`os/src/mm/global_allocator/global_allocator.rs:35`

### 使用

初始化后自动支持 `alloc` crate：

```rust
use alloc::vec::Vec;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::collections::BTreeMap;

let v = Vec::new();
let b = Box::new(42);
let s = String::from("hello");
let m = BTreeMap::new();
```

---

## 页表抽象层 API (page_table/)

### PageTableInner trait

```rust
pub trait PageTableInner<T: PageTableEntry> {
    // 常量
    const LEVELS: usize;
    const MAX_VA_BITS: usize;
    const MAX_PA_BITS: usize;

    // 生命周期
    fn new() -> Self;
    fn from_ppn(ppn: Ppn) -> Self;
    fn activate(ppn: Ppn);
    fn root_ppn(&self) -> Ppn;

    // TLB管理
    fn tlb_flush(vpn: Vpn);
    fn tlb_flush_all();

    // 映射操作
    fn map(&mut self, vpn: Vpn, ppn: Ppn, page_size: PageSize,
           flags: UniversalPTEFlag) -> PagingResult<()>;
    fn unmap(&mut self, vpn: Vpn) -> PagingResult<()>;
    fn remap(&mut self, vpn: Vpn, new_ppn: Ppn, page_size: PageSize,
             flags: UniversalPTEFlag) -> PagingResult<()>;

    // 查询
    fn translate(&self, vaddr: Vaddr) -> Option<Paddr>;
    fn walk(&self, vpn: Vpn) -> PagingResult<(Ppn, PageSize, UniversalPTEFlag)>;
}
```

**源代码**：`os/src/mm/page_table/page_table.rs:5-52`

### UniversalPTEFlag

```rust
impl UniversalPTEFlag {
    // 基础标志
    pub const V: Self;  // Valid
    pub const R: Self;  // Readable
    pub const W: Self;  // Writable
    pub const X: Self;  // Executable
    pub const U: Self;  // User
    pub const G: Self;  // Global
    pub const A: Self;  // Accessed
    pub const D: Self;  // Dirty

    // 预定义组合
    pub fn user_read() -> Self;      // U | R | V
    pub fn user_rw() -> Self;        // U | R | W | V
    pub fn user_rx() -> Self;        // U | R | X | V
    pub fn kernel_r() -> Self;       // R | V
    pub fn kernel_rw() -> Self;      // R | W | V
}
```

**源代码**：`os/src/mm/page_table/page_table_entry.rs:4-58`

### PageSize

```rust
pub enum PageSize {
    Size4K = 0x1000,
    Size2M = 0x20_0000,    // 暂时禁用
    Size1G = 0x4000_0000,  // 暂时禁用
}
```

### PagingError

```rust
pub enum PagingError {
    NotMapped,
    AlreadyMapped,
    InvalidAddress,
    InvalidPageSize,
    PermissionDenied,
    PageTableFull,
    FrameAllocationFailed,
    // ... 更多
}
```

**源代码**：`os/src/mm/page_table/mod.rs:23-44`

---

## 地址空间管理 API (memory_space/)

### MemorySpace

#### 创建

```rust
impl MemorySpace {
    pub fn new_kernel() -> Self              // 创建内核地址空间
    pub fn from_elf(elf_data: &[u8]) -> Self // 从ELF创建用户地址空间
}
```

**源代码**：`os/src/mm/memory_space/memory_space.rs:203-458`

#### 系统调用支持

```rust
pub fn brk(&mut self, new_end: Vaddr) -> SyscallResult<Vaddr>
pub fn mmap(&mut self, start: Vaddr, len: usize, prot: usize) -> SyscallResult<Vaddr>
pub fn munmap(&mut self, start: Vaddr, len: usize) -> SyscallResult<()>
```

#### 进程管理

```rust
pub fn clone_for_fork(&self) -> Self           // fork时深拷贝
pub fn activate(&self)                          // 激活地址空间
pub fn root_ppn(&self) -> Ppn                   // 获取根页表页号
```

### MappingArea

#### 创建

```rust
impl MappingArea {
    pub fn new(
        vaddr_range: VaddrRange,
        map_type: MapType,
        permission: UniversalPTEFlag,
        area_type: AreaType,
    ) -> Self
}
```

**源代码**：`os/src/mm/memory_space/mapping_area.rs:62-100`

#### 映射类型

```rust
pub enum MapType {
    Direct,   // 直接映射（内核）
    Framed,   // 帧映射（用户）
}

pub enum AreaType {
    KernelText,
    KernelData,
    UserText,
    UserData,
    UserStack,
    UserHeap,
    // ...
}
```

#### 操作

```rust
pub fn map(&mut self, page_table: &mut ActivePageTableInner) -> PagingResult<()>
pub fn unmap(&mut self, page_table: &mut ActivePageTableInner) -> PagingResult<()>
pub fn copy_data(&self, page_table: &ActivePageTableInner, data: &[u8])
pub fn extend(&mut self, page_table: &mut ActivePageTableInner,
              new_end_vpn: Vpn) -> PagingResult<()>
pub fn shrink(&mut self, page_table: &mut ActivePageTableInner,
              new_end_vpn: Vpn) -> PagingResult<()>
```

**源代码**：`os/src/mm/memory_space/mapping_area.rs:113-626`

---

## 架构特定 API (arch/*/mm/)

### RISC-V (arch/riscv/mm/)

#### 地址转换

```rust
pub const unsafe fn vaddr_to_paddr(vaddr: usize) -> usize
pub const fn paddr_to_vaddr(paddr: usize) -> usize
```

**源代码**：`os/src/arch/riscv/mm/mod.rs:8-15`

#### 常量

```rust
pub const VADDR_START: usize = 0xffff_ffc0_0000_0000;
pub const PADDR_MASK: usize = 0x0000_003f_ffff_ffff;
```

#### SV39 PageTableInner

```rust
impl PageTableInner<PageTableEntry> for PageTableInner {
    const LEVELS: usize = 3;
    const MAX_VA_BITS: usize = 39;
    const MAX_PA_BITS: usize = 56;
    // ... trait实现
}
```

**源代码**：`os/src/arch/riscv/mm/page_table.rs:21-286`

---

## 配置常量 (config.rs)

```rust
// 基础配置
pub const PAGE_SIZE: usize = 4096;
pub const KERNEL_HEAP_SIZE: usize = 16 * 1024 * 1024;  // 16 MB
pub const USER_STACK_SIZE: usize = 4 * 1024 * 1024;    // 4 MB
pub const MAX_USER_HEAP_SIZE: usize = 64 * 1024 * 1024; // 64 MB

// 内存布局
pub const TRAMPOLINE: usize = usize::MAX - PAGE_SIZE + 1;
pub const TRAP_CONTEXT: usize = TRAMPOLINE - 2 * PAGE_SIZE;
pub const USER_STACK_TOP: usize = TRAP_CONTEXT - PAGE_SIZE;

// 平台相关
pub const MEMORY_END: usize = 0x88000000;  // 128 MB
```

**源代码**：`os/src/config.rs`

---

## 常用模式

### 模式 1：分配并映射页面

```rust
use crate::mm::frame_allocator::alloc_frame;
use crate::mm::page_table::PageSize;

let frame = alloc_frame()?;
let ppn = frame.ppn();
page_table.map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::user_rw())?;
frames.insert(vpn, TrackedFrames::Single(frame));
```

### 模式 2：创建用户地址空间

```rust
use crate::mm::memory_space::MemorySpace;

let elf_data = load_elf_from_disk(path)?;
let mut space = MemorySpace::from_elf(&elf_data);
space.activate();
```

### 模式 3：扩展堆区域

```rust
let new_end = current_end + size;
let new_end_vaddr = Vaddr::new(new_end);
space.brk(new_end_vaddr)?;
```

### 模式 4：地址转换

```rust
// 虚拟地址 → 物理地址
let vaddr = Vaddr::new(0xffff_ffc0_8000_1000);
let paddr = page_table.translate(vaddr).unwrap();

// 页号 → 地址
let vpn = Vpn::new(0x100);
let vaddr = vpn.start_addr();
```

---

## 快速索引

| 功能 | API | 源文件 |
|------|-----|--------|
| 初始化MM | `mm::init()` | `mm/mod.rs:33` |
| 分配单帧 | `alloc_frame()` | `frame_allocator/frame_allocator.rs:219` |
| 分配连续帧 | `alloc_contig_frames(n)` | `frame_allocator/frame_allocator.rs:223` |
| 地址对齐 | `addr.align_up_to_page()` | `address/operations.rs:69` |
| 页号转换 | `Ppn::from_addr_floor(paddr)` | `address/page_num.rs:34` |
| 创建页表 | `PageTableInner::new()` | `arch/riscv/mm/page_table.rs:56` |
| 映射页面 | `page_table.map(vpn, ppn, ...)` | `page_table/page_table.rs:35` |
| 创建用户空间 | `MemorySpace::from_elf(data)` | `memory_space/memory_space.rs:353` |
| 扩展堆 | `space.brk(new_end)` | `memory_space/memory_space.rs:461` |
| 地址转换 | `vaddr.to_paddr()` | `address/address.rs:50` |

---

## 版本信息

- **文档版本**：1.0
- **Rust工具链**：nightly-2025-01-13
- **支持架构**：RISC-V (SV39), LoongArch (TODO)
- **页面大小**：4KB（大页支持已暂时禁用）

---

## 相关文档

- **[总览](README.md)** - MM子系统简介和导航
- **[架构设计](architecture.md)** - 分层架构和设计决策
- **[地址抽象层](address/overview.md)** - Paddr/Vaddr/Ppn/Vpn详解
- **[物理帧分配器](frame_allocator/overview.md)** - FrameAllocator详解
- **[内核堆分配器](global_allocator/heap_allocator.md)** - talc全局分配器
- **[页表抽象层](page_table/overview.md)** - PageTableInner trait
- **[地址空间管理](memory_space/overview.md)** - MemorySpace详解

## 在线文档

完整的 rustdoc 文档：

```bash
cd os && cargo doc --open
```

生成的文档位于：`target/doc/os/mm/index.html`
