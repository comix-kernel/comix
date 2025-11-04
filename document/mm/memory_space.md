# 地址空间管理

## 概述

地址空间管理是 MM 子系统的最高抽象层，负责管理整个虚拟地址空间的布局、映射区域和页表操作。每个进程拥有独立的虚拟地址空间，支持内核和用户态的内存隔离。

### 设计目标

1. **地址空间隔离**：每个进程拥有独立的虚拟地址空间
2. **灵活的内存布局**：支持代码段、数据段、堆、栈等多种区域
3. **按需分配**：延迟分配物理内存，节省资源
4. **系统调用支持**：实现 brk、mmap、munmap 等内存管理系统调用

## 核心结构

### MemorySpace

```rust
pub struct MemorySpace {
    page_table: ActivePageTableInner,    // 页表（管理虚拟→物理映射）
    areas: Vec<MappingArea>,             // 映射区域列表
    heap_top: Option<Vpn>,               // 用户堆顶（brk）
}
```

### MappingArea

```rust
pub struct MappingArea {
    vpn_range: VpnRange,                 // 虚拟页号范围 [start, end)
    area_type: AreaType,                 // 区域类型（代码/数据/堆/栈）
    map_type: MapType,                   // 映射策略（Direct/Framed）
    permission: UniversalPTEFlag,        // 权限标志（R/W/X/U）
    frames: BTreeMap<Vpn, TrackedFrames>,  // 物理帧映射（Framed 类型使用）
}
```

## 映射策略

### Direct 直接映射

用于内核空间，虚拟地址直接对应物理地址：

```
虚拟地址                    物理地址
0xFFFF_FFC0_8000_0000  ←→  0x8000_0000
0xFFFF_FFC0_8000_1000  ←→  0x8000_1000

特点:
✓ 无需分配物理帧
✓ 访问物理内存无需查页表
✓ 仅用于内核空间
```

```rust
// 实现
for vpn in self.vpn_range {
    let vaddr = vpn.start_addr();
    let paddr = vaddr.to_paddr();
    let ppn = Ppn::from_addr_floor(paddr);
    page_table.map(vpn, ppn, PageSize::Size4K, self.permission)?;
}
```

### Framed 帧映射

用于用户空间，每个虚拟页分配独立的物理帧：

```
虚拟页号      物理页号
VPN 0x1000  ←→  PPN 0x8234_5  (分配)
VPN 0x1001  ←→  PPN 0x8456_7  (分配)

特点:
✓ 每个虚拟页分配独立物理帧
✓ 物理内存可能不连续
✓ 自动管理物理帧生命周期（RAII）
```

```rust
// 实现
for vpn in self.vpn_range {
    let frame = alloc_frame()?;
    let ppn = frame.ppn();
    page_table.map(vpn, ppn, PageSize::Size4K, self.permission)?;
    self.frames.insert(vpn, TrackedFrames::Single(frame));
}
```

## 地址空间创建

### 内核地址空间

```rust
pub fn new_kernel() -> Self {
    let mut space = Self {
        page_table: ActivePageTableInner::new(),
        areas: Vec::new(),
        heap_top: None,
    };

    // 1. 映射跳板页
    space.map_trampoline();

    // 2. 映射内核各段（Direct 映射）
    space.map_kernel_text();    // .text   R+X
    space.map_kernel_rodata();  // .rodata R
    space.map_kernel_data();    // .data   R+W
    space.map_kernel_bss();     // .bss    R+W
    space.map_kernel_heap();    // heap    R+W

    // 3. 直接映射物理内存
    space.map_physical_memory();

    space
}
```

**内核段映射示例**：

```rust
// .text 段（只读可执行）
let text_start = Vaddr::new(stext as usize);
let text_end = Vaddr::new(etext as usize);
space.push(MappingArea::new(
    VaddrRange::new(text_start, text_end),
    MapType::Direct,
    UniversalPTEFlag::kernel_r() | UniversalPTEFlag::X,
    AreaType::KernelText,
));
```

### 用户地址空间

```rust
pub fn from_elf(elf_data: &[u8]) -> Self {
    let mut space = Self {
        page_table: ActivePageTableInner::new(),
        areas: Vec::new(),
        heap_top: None,
    };

    // 1. 解析 ELF 文件，映射各段（Framed 映射）
    let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
    for program_header in elf.program_iter() {
        if program_header.get_type() == Ok(xmas_elf::program::Type::Load) {
            // 创建映射区域
            let area = MappingArea::new(
                vaddr_range,
                MapType::Framed,
                permission,
                area_type,
            );
            // 拷贝数据到物理页
            area.copy_data(&space.page_table, program_header.get_data(&elf).unwrap());
            space.areas.push(area);
        }
    }

    // 2. 映射用户栈
    space.map_user_stack();

    // 3. 映射 trap 上下文
    space.map_trap_context();

    // 4. 初始化堆
    space.heap_top = Some(space.infer_heap_start());

    space
}
```

## 系统调用支持

### brk - 堆扩展

```rust
pub fn brk(&mut self, new_end: Vaddr) -> SyscallResult<Vaddr> {
    let new_end_vpn = Vpn::from_addr_ceil(new_end);
    let current_end_vpn = self.heap_top.unwrap();

    if new_end_vpn > current_end_vpn {
        // 扩展堆
        let heap_area = self.find_heap_area_mut()?;
        heap_area.extend(&mut self.page_table, new_end_vpn)?;
        self.heap_top = Some(new_end_vpn);
    } else if new_end_vpn < current_end_vpn {
        // 收缩堆
        let heap_area = self.find_heap_area_mut()?;
        heap_area.shrink(&mut self.page_table, new_end_vpn)?;
        self.heap_top = Some(new_end_vpn);
    }

    Ok(new_end)
}
```

**使用场景**：

```rust
// C 标准库 malloc 底层调用
let old_brk = process.memory_space.brk(Vaddr::new(0))?;  // 获取当前堆顶
let new_brk = old_brk + Vaddr::new(size);
process.memory_space.brk(new_brk)?;  // 扩展堆
```

### mmap - 匿名内存映射

```rust
pub fn mmap(&mut self, start: Vaddr, len: usize, prot: usize) -> SyscallResult<Vaddr> {
    let start_vpn = Vpn::from_addr_floor(start);
    let end_vpn = Vpn::from_addr_ceil(start + Vaddr::new(len));

    // 检查地址范围是否可用
    self.check_range_available(VpnRange::new(start_vpn, end_vpn))?;

    // 创建新的映射区域
    let permission = Self::prot_to_pte_flags(prot);
    let area = MappingArea::new(
        VaddrRange::new(start, start + Vaddr::new(len)),
        MapType::Framed,
        permission,
        AreaType::UserAnonymous,
    );

    // 映射到页表
    area.map(&mut self.page_table)?;
    self.areas.push(area);

    Ok(start)
}
```

**使用场景**：

```rust
// 用户程序请求匿名内存
let addr = mmap(NULL, 4096, PROT_READ | PROT_WRITE,
                MAP_ANONYMOUS | MAP_PRIVATE, -1, 0);
```

### munmap - 取消映射

```rust
pub fn munmap(&mut self, start: Vaddr, len: usize) -> SyscallResult<()> {
    let start_vpn = Vpn::from_addr_floor(start);
    let end_vpn = Vpn::from_addr_ceil(start + Vaddr::new(len));
    let unmap_range = VpnRange::new(start_vpn, end_vpn);

    // 找到重叠的映射区域并取消映射
    let mut areas_to_remove = Vec::new();
    for (idx, area) in self.areas.iter().enumerate() {
        if area.vpn_range.intersects(&unmap_range) {
            areas_to_remove.push(idx);
        }
    }

    // 取消映射并释放资源
    for idx in areas_to_remove.iter().rev() {
        let area = self.areas.remove(*idx);
        area.unmap(&mut self.page_table)?;
    }

    Ok(())
}
```

## 进程管理

### fork 时的地址空间复制

```rust
pub fn clone_for_fork(&self) -> Self {
    let mut new_space = Self {
        page_table: ActivePageTableInner::new(),
        areas: Vec::new(),
        heap_top: self.heap_top,
    };

    // 深拷贝所有映射区域
    for area in &self.areas {
        let mut new_area = area.clone_structure();

        // 拷贝物理页内容
        for vpn in area.vpn_range {
            if let Some(old_frame) = area.frames.get(&vpn) {
                let new_frame = old_frame.clone();  // 分配新帧并拷贝数据
                new_area.frames.insert(vpn, new_frame);
            }
        }

        // 映射到新页表
        new_area.map(&mut new_space.page_table)?;
        new_space.areas.push(new_area);
    }

    new_space
}
```

**写时复制（COW）优化**（未来改进）：

fork 时共享物理页并标记为只读，写入时触发缺页异常再复制，可显著提升性能并减少内存占用。

### 激活地址空间

```rust
pub fn activate(&self) {
    let root_ppn = self.page_table.root_ppn();
    ActivePageTableInner::activate(root_ppn);
}
```

**使用场景**：

```rust
// 进程切换
fn switch_to_process(process: &mut Process) {
    process.memory_space.activate();  // 切换页表
    // 跳转到用户态
}
```

## 区域类型

```rust
pub enum AreaType {
    KernelText,       // 内核代码段
    KernelData,       // 内核数据段
    KernelHeap,       // 内核堆
    UserText,         // 用户代码段
    UserData,         // 用户数据段
    UserHeap,         // 用户堆
    UserStack,        // 用户栈
    UserAnonymous,    // 用户匿名映射（mmap）
    Trampoline,       // 跳板页
    TrapContext,      // Trap 上下文
}
```

## 使用场景

### 场景 1：创建新进程

```rust
// 从 ELF 文件加载程序
let elf_data = load_elf_from_disk("/bin/hello")?;
let memory_space = MemorySpace::from_elf(&elf_data);

let process = Process {
    memory_space,
    // ... 其他字段
};
```

### 场景 2：进程 fork

```rust
fn sys_fork() -> SyscallResult<Pid> {
    let parent = current_process();
    let child_space = parent.memory_space.clone_for_fork();

    let child = Process {
        memory_space: child_space,
        parent: Some(parent.pid()),
        // ... 其他字段
    };

    Ok(child.pid())
}
```

### 场景 3：动态内存分配（brk）

```rust
fn sys_brk(new_brk: usize) -> SyscallResult<usize> {
    let process = current_process_mut();
    let new_end = Vaddr::new(new_brk);
    process.memory_space.brk(new_end)?;
    Ok(new_brk)
}
```

### 场景 4：内存映射（mmap）

```rust
fn sys_mmap(start: usize, len: usize, prot: usize) -> SyscallResult<usize> {
    let process = current_process_mut();
    let start_vaddr = Vaddr::new(start);
    let mapped_addr = process.memory_space.mmap(start_vaddr, len, prot)?;
    Ok(mapped_addr.as_usize())
}
```

## 常见问题

### Q1: 内核和用户地址空间如何隔离？

**A**: 通过虚拟地址范围和 U 标志位：
- 内核空间：高半核（0xffff_ffc0_0000_0000 以上），U=0
- 用户空间：低地址（0x0 开始），U=1
- CPU 在用户态无法访问 U=0 的页面

### Q2: fork 时为什么要深拷贝？

**A**: 当前实现保证父子进程完全独立。未来可改用写时复制（COW）优化性能。

### Q3: 如何防止用户程序访问内核内存？

**A**: 两层保护：
1. 页表权限：内核页面设置 U=0
2. 地址检查：系统调用参数验证用户指针合法性

### Q4: mmap 分配的地址范围如何选择？

**A**: 当前实现要求用户指定地址。未来可实现地址分配器自动选择空闲区域。

## 性能优化

### TLB 刷新优化

```rust
// ✅ 高效：批量映射后一次性刷新
for vpn in vpn_range {
    area.map_single_page(vpn, &mut page_table)?;
}
PageTableInner::tlb_flush_all();
```

### 内存占用优化

- 共享只读页面（代码段、只读数据）
- 写时复制（COW）
- 大页支持（减少页表级数）
- 延迟分配（按需分配物理帧）

## 相关文档

- [地址抽象层](address.md) - Vaddr/Vpn 类型
- [页表抽象层](page_table.md) - 页表操作接口
- [物理帧分配器](frame_allocator.md) - 物理内存分配
- [整体架构](architecture.md) - MM 子系统架构
- [API 参考](api_reference.md) - API 快速查询

## 参考实现

- **源代码**：`os/src/mm/memory_space/`
- **初始化**：`os/src/mm/mod.rs:47-51`（内核地址空间）
