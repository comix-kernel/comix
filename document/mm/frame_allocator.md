# 物理帧分配器

## 概述

物理帧分配器（Frame Allocator）负责管理可用物理内存页面（帧）的分配和回收。采用**水位线 + 回收栈**的混合策略，平衡了分配效率和内存利用率。

### 设计目标

1. **高效分配**：O(1) 时间复杂度分配单帧和连续帧
2. **自动回收**：通过 RAII 机制防止内存泄漏
3. **减少碎片**：回收栈自动合并连续帧
4. **支持对齐**：满足 DMA 等场景的对齐需求

### 核心组件

- **FrameAllocator**：全局分配器，管理物理帧池
- **FrameTracker**：单帧 RAII 包装器
- **FrameRangeTracker**：连续帧 RAII 包装器
- **TrackedFrames**：统一的帧枚举类型

## 分配器原理

### 数据结构

```rust
pub struct FrameAllocator {
    start: Ppn,           // 可分配区域起始页号
    end: Ppn,             // 可分配区域结束页号（左闭右开）
    cur: Ppn,             // 当前分配水位线
    recycled: Vec<Ppn>,   // 回收栈（按升序存储已释放的页号）
}
```

### 分配策略

```
物理内存布局:

0x8000_0000                                    MEMORY_END
    │                                              │
    ▼                                              ▼
    ┌──────────────┬───────────────────────────────┬─────┐
    │  内核占用    │    可分配区域 [start, end)     │未用 │
    └──────────────┴───────────────────────────────┴─────┘
                    ↑                    ↑          ↑
                  start                 cur        end

分配顺序:
1. 优先从回收栈分配（LIFO）
2. 回收栈为空时从水位线分配
3. 水位线递增
```

### 回收优化

回收时自动检测并合并栈顶连续帧：

```
场景：按相反顺序释放连续帧

初始: cur = 105, recycled = []

1. dealloc_frame(104):
   recycled = [104]
   104 + 1 == 105 (cur) → 合并！
   recycled = [], cur = 104

2. dealloc_frame(103):
   recycled = [103]
   103 + 1 == 104 (cur) → 合并！
   recycled = [], cur = 103

最终: cur = 102, recycled = []  // 完全回收
```

## 核心 API

### 单帧分配

```rust
// 分配单个物理帧
let frame = alloc_frame()?;
let ppn = frame.ppn();

// 访问帧内存
let bytes = frame.as_slice_mut::<u8>();
bytes[0] = 0xff;

// FrameTracker 离开作用域时自动释放
```

### 多帧分配（非连续）

```rust
// 分配 5 个帧（可能非连续）
let frames = alloc_frames(5)?;

for frame in &frames {
    println!("Allocated PPN: {:#x}", frame.ppn().as_usize());
}
// frames 离开作用域时批量释放
```

### 连续帧分配

```rust
// 分配 256 个连续帧（1MB）
let contig = alloc_contig_frames(256)?;

assert_eq!(contig.len(), 256);
let start_ppn = contig.start_ppn();
let end_ppn = contig.end_ppn();  // 左闭右开

// 遍历连续帧
for ppn in contig.iter() {
    println!("PPN: {:#x}", ppn.as_usize());
}
```

### 对齐连续帧分配

```rust
// 分配 512 个 4KB 页（2MB），起始地址 2MB 对齐
let ppn_per_2mb = 512;
let huge_page = alloc_contig_frames_aligned(512, ppn_per_2mb)?;

// 验证对齐
assert_eq!(huge_page.start_ppn().as_usize() % ppn_per_2mb, 0);
```

## RAII 机制

### FrameTracker

单帧的 RAII 包装器，离开作用域时自动释放：

```rust
pub struct FrameTracker {
    ppn: Ppn,
}

impl FrameTracker {
    pub fn ppn(&self) -> Ppn { self.ppn }

    // 访问帧内存
    pub fn as_slice<T>(&self) -> &[T] { /* ... */ }
    pub fn as_slice_mut<T>(&mut self) -> &mut [T] { /* ... */ }
}

impl Drop for FrameTracker {
    fn drop(&mut self) {
        dealloc_frame(self.ppn);  // 自动释放
    }
}

impl Clone for FrameTracker {
    fn clone(&self) -> Self {
        // 克隆时分配新帧并拷贝内容
        alloc_frame().unwrap()
    }
}
```

**使用示例**：

```rust
{
    let frame = alloc_frame()?;
    // 使用 frame
}  // 自动释放

// 避免过早释放
fn wrong_usage() -> Result<(), FrameAllocError> {
    // ❌ 错误：过早释放
    let ppn = {
        let frame = alloc_frame()?;
        frame.ppn()
    };  // frame 被释放
    page_table.map(vpn, ppn, ...)?;  // 映射已释放的帧！

    Ok(())
}

fn correct_usage() -> Result<(), FrameAllocError> {
    // ✅ 正确：延长生命周期
    let frame = alloc_frame()?;
    let ppn = frame.ppn();
    page_table.map(vpn, ppn, ...)?;
    frames.push(frame);  // 存储以保持所有权

    Ok(())
}
```

### FrameRangeTracker

连续帧的 RAII 包装器：

```rust
pub struct FrameRangeTracker {
    start_ppn: Ppn,
    end_ppn: Ppn,  // 左闭右开
}

impl FrameRangeTracker {
    pub fn start_ppn(&self) -> Ppn { self.start_ppn }
    pub fn end_ppn(&self) -> Ppn { self.end_ppn }
    pub fn len(&self) -> usize { /* ... */ }

    // 迭代所有页号
    pub fn iter(&self) -> impl Iterator<Item = Ppn> { /* ... */ }
}

impl Drop for FrameRangeTracker {
    fn drop(&mut self) {
        // 批量释放所有连续帧
        for ppn in self.iter() {
            dealloc_frame(ppn);
        }
    }
}
```

### TrackedFrames 枚举

统一的帧枚举类型，用于映射区域：

```rust
pub enum TrackedFrames {
    Single(FrameTracker),
    Multiple(Vec<FrameTracker>),
    Contiguous(FrameRangeTracker),
}

impl TrackedFrames {
    pub fn count(&self) -> usize {
        match self {
            Self::Single(_) => 1,
            Self::Multiple(v) => v.len(),
            Self::Contiguous(r) => r.len(),
        }
    }

    pub fn ppns(&self) -> impl Iterator<Item = Ppn> + '_ {
        match self {
            Self::Single(f) => /* ... */,
            Self::Multiple(v) => /* ... */,
            Self::Contiguous(r) => r.iter(),
        }
    }
}
```

**使用场景**：

```rust
// MappingArea 中存储不同类型的帧
pub struct MappingArea {
    vpn_range: VpnRange,
    frames: BTreeMap<Vpn, TrackedFrames>,  // 灵活存储
    // ...
}

impl MappingArea {
    pub fn push_single(&mut self, vpn: Vpn) {
        let frame = alloc_frame().unwrap();
        self.frames.insert(vpn, TrackedFrames::Single(frame));
    }

    pub fn push_contig(&mut self, vpn_range: VpnRange) {
        let contig = alloc_contig_frames(vpn_range.len()).unwrap();
        let start_vpn = vpn_range.start();
        self.frames.insert(start_vpn, TrackedFrames::Contiguous(contig));
    }
}
```

## 初始化

```rust
// os/src/mm/mod.rs:36-41
pub fn init() {
    // 计算可用物理内存范围
    let ekernel_paddr = unsafe { vaddr_to_paddr(ekernel as usize) };
    let start = Ppn::from_addr_ceil(Paddr::new(ekernel_paddr));
    let end = Ppn::from_addr_floor(Paddr::new(MEMORY_END));

    // 初始化全局帧分配器
    init_frame_allocator(start, end);
}
```

## 错误处理

```rust
#[derive(Debug)]
pub enum FrameAllocError {
    OutOfMemory,     // 物理内存耗尽
    InvalidAddress,  // 地址无效
    AlignmentError,  // 对齐错误
}

pub type FrameAllocResult<T> = Result<T, FrameAllocError>;
```

**常见错误场景**：

```rust
// OutOfMemory：物理内存耗尽
match alloc_frame() {
    Ok(frame) => { /* 使用 frame */ },
    Err(FrameAllocError::OutOfMemory) => {
        panic!("Physical memory exhausted!");
    }
}

// AlignmentError：对齐值不是 2 的幂
let result = alloc_contig_frames_aligned(10, 15);  // 15 不是 2 的幂
assert!(matches!(result, Err(FrameAllocError::AlignmentError)));
```

## 使用场景

### 场景 1：页表创建

```rust
// 分配页表根页面
let root_frame = alloc_frame()?;
let root_ppn = root_frame.ppn();

// 初始化页表
let page_table = PageTableInner::from_ppn(root_ppn);

// root_frame 需要保持所有权，直到页表销毁
```

### 场景 2：用户程序加载

```rust
pub fn load_elf(&mut self, elf_data: &[u8]) -> Result<(), ElfError> {
    let elf = xmas_elf::ElfFile::new(elf_data)?;

    for ph in elf.program_iter() {
        if ph.get_type() != ProgramHeaderType::Load {
            continue;
        }

        let start_vpn = Vpn::from_addr_floor(Vaddr::new(ph.virtual_addr() as usize));
        let end_vpn = Vpn::from_addr_ceil(Vaddr::new(
            (ph.virtual_addr() + ph.mem_size()) as usize
        ));

        // 为每个页分配物理帧
        for vpn in VpnRange::new(start_vpn, end_vpn) {
            let frame = alloc_frame()?;
            let ppn = frame.ppn();

            // 映射
            self.page_table.map(vpn, ppn, PageSize::Size4K, flags)?;

            // 拷贝数据
            let dst = ppn.start_addr().to_vaddr().as_usize() as *mut u8;
            // ...

            // 存储 frame 以保持所有权
            self.frames.insert(vpn, TrackedFrames::Single(frame));
        }
    }

    Ok(())
}
```

### 场景 3：DMA 缓冲区

```rust
// 分配 4MB DMA 缓冲区（1024 个 4KB 页，4MB 对齐）
let dma_pages = 1024;
let alignment = 1024;  // 4MB = 1024 * 4KB

let dma_buffer = alloc_contig_frames_aligned(dma_pages, alignment)?;

// 传递物理地址给 DMA 控制器
let dma_paddr = dma_buffer.start_ppn().start_addr();
configure_dma(dma_paddr.as_usize());
```

## 常见陷阱

### 陷阱 1：忘记 forget

```rust
// ❌ 错误：重复释放
pub fn manual_dealloc(frame: FrameTracker) {
    dealloc_frame(frame.ppn());
    // frame Drop 时会再次释放！
}

// ✅ 正确：手动释放后 forget
pub fn manual_dealloc(frame: FrameTracker) {
    dealloc_frame(frame.ppn());
    core::mem::forget(frame);  // 防止 Drop
}
```

### 陷阱 2：过早释放

参见前文 FrameTracker 使用示例。

### 陷阱 3：Clone 语义误解

```rust
// Clone 会分配新帧并拷贝内容
let frame1 = alloc_frame()?;
let frame2 = frame1.clone();  // 分配新帧！

assert_ne!(frame1.ppn(), frame2.ppn());  // 不同的物理帧
```

## 调试技巧

```rust
// 查看分配器状态
with_frame_allocator(|allocator| {
    println!("Total frames: {}", allocator.end.as_usize() - allocator.start.as_usize());
    println!("Allocated: {}", allocator.cur.as_usize() - allocator.start.as_usize());
    println!("Recycled: {}", allocator.recycled.len());
});
```

## 相关文档

- [地址抽象层](address.md) - Ppn/Paddr 类型
- [整体架构](architecture.md) - MM 子系统架构
- [页表抽象层](page_table.md) - 帧的映射使用
- [API 参考](api_reference.md) - 完整 API 列表

## 参考实现

- **源代码**：`os/src/mm/frame_allocator/`
- **初始化**：`os/src/mm/mod.rs:36-41`
