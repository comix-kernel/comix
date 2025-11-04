# 地址抽象层

## 概述

地址抽象层提供了类型安全的物理地址和虚拟地址抽象，以及页号（Page Number）的相关操作。通过 `repr(transparent)` 实现零成本抽象，同时提供编译期类型安全保证。

### 设计目标

1. **类型安全**：物理地址和虚拟地址使用不同类型，防止混用
2. **零成本抽象**：通过 `repr(transparent)` 保证与 usize 相同的内存布局
3. **便捷操作**：提供丰富的算术运算、对齐操作和范围查询
4. **架构无关**：地址转换逻辑委托给架构特定层

### 核心类型

```rust
// 物理地址和虚拟地址
#[repr(transparent)]
pub struct Paddr(usize);

#[repr(transparent)]
pub struct Vaddr(usize);

// 物理页号和虚拟页号
#[repr(transparent)]
pub struct Ppn(usize);

#[repr(transparent)]
pub struct Vpn(usize);

// 地址范围（左闭右开区间）
pub struct AddressRange<T: Address> {
    start: T,  // 包含
    end: T,    // 不包含
}

pub type PaddrRange = AddressRange<Paddr>;
pub type VaddrRange = AddressRange<Vaddr>;
pub type PpnRange = AddressRange<Ppn>;
pub type VpnRange = AddressRange<Vpn>;
```

## 地址类型 (Paddr/Vaddr)

### 基本操作

```rust
// 创建地址
let paddr = Paddr::new(0x8000_0000);
let vaddr = Vaddr::new(0xffff_ffc0_8000_0000);

// 转换为 usize
let addr_val: usize = paddr.as_usize();

// 地址转换
let vaddr = paddr.to_vaddr();  // 物理 → 虚拟
let paddr = vaddr.to_paddr();  // 虚拟 → 物理（unsafe）
```

### 算术运算

```rust
// 基于类型大小的运算
let addr = Vaddr::new(0x1000);
let addr2 = addr.add::<u64>(3);  // 0x1000 + 3 * 8 = 0x1018

// 单步前进/后退
let next = addr.step();       // 0x1001
let prev = addr.step_back();  // 0x0fff

// 运算符重载
let sum = addr1 + addr2;      // 加法
let diff = addr1 - addr2;     // 减法
let aligned = addr1 & mask;   // 位与（用于对齐）
```

### 对齐操作

```rust
let addr = Vaddr::new(0x1234);

// 按任意对齐值对齐
let up = addr.align_up(16);       // 0x1240
let down = addr.align_down(16);   // 0x1230
assert!(addr.is_aligned(16));     // false

// 按页对齐（PAGE_SIZE = 4096）
let page_aligned = addr.align_up_to_page();  // 0x2000
assert!(page_aligned.is_page_aligned());     // true
```

### 地址范围

```rust
// 创建范围 [0x1000, 0x5000)
let range = VaddrRange::new(
    Vaddr::new(0x1000),
    Vaddr::new(0x5000)
);

// 长度和边界
assert_eq!(range.len(), 0x4000);
assert_eq!(range.start(), Vaddr::new(0x1000));
assert_eq!(range.end(), Vaddr::new(0x5000));

// 包含关系（注意：左闭右开）
assert!(range.contains(&Vaddr::new(0x1000)));   // 包含 start
assert!(!range.contains(&Vaddr::new(0x5000)));  // 不包含 end

// 区间运算
let r1 = VaddrRange::new(Vaddr::new(0x1000), Vaddr::new(0x3000));
let r2 = VaddrRange::new(Vaddr::new(0x2000), Vaddr::new(0x4000));

if r1.intersects(&r2) {
    let inter = r1.intersection(&r2).unwrap();  // [0x2000, 0x3000)
}

let union = r1.union(&r2).unwrap();  // [0x1000, 0x4000)
```

## 页号类型 (Ppn/Vpn)

### 地址与页号转换

```rust
// 地址 → 页号
let addr = Paddr::new(0x8000_1234);
let ppn_floor = Ppn::from_addr_floor(addr);  // 向下取整：0x8000_1234 / 4096
let ppn_ceil = Ppn::from_addr_ceil(addr);    // 向上取整

// 页号 → 地址
let vpn = Vpn::new(0x100);
let start_addr = vpn.start_addr();  // 页起始地址：0x100 * 4096
let end_addr = vpn.end_addr();      // 下一页起始地址：0x101 * 4096
```

### 页号运算

```rust
let ppn = Ppn::new(0x8000_1);

// 步进
let next_ppn = ppn.step();       // 0x8000_2
let prev_ppn = ppn.step_back();  // 0x8000_0

// 偏移
let offset_ppn = ppn.offset(10);  // 0x8000_b
```

### 页号范围

```rust
// 创建范围 [0x10, 0x20)
// 注意：左闭右开
let range = VpnRange::new(Vpn::new(0x10), Vpn::new(0x20));

assert_eq!(range.len(), 0x10);  // 包含 16 个页

// 迭代页号
for vpn in range {
    println!("VPN: {:#x}", vpn.as_usize());
}
// 输出：0x10, 0x11, ..., 0x1f（不包含 0x20）
```

## 地址运算 Trait

地址类型通过以下 trait 提供统一的操作接口：

### UsizeConvert

```rust
pub trait UsizeConvert {
    fn as_usize(&self) -> usize;
    fn from_usize(value: usize) -> Self;
}
```

### CalcOps

提供算术和位运算：

```rust
// 支持的运算符
addr1 + addr2   // 加法
addr1 - addr2   // 减法
addr1 & mask    // 位与
addr1 | mask    // 位或
addr1 ^ mask    // 位异或
addr1 >> n      // 右移
addr1 << n      // 左移
```

### AlignOps

提供对齐操作：

```rust
pub trait AlignOps {
    fn is_aligned(&self, align: usize) -> bool;
    fn align_up(&self, align: usize) -> Self;
    fn align_down(&self, align: usize) -> Self;
    fn is_page_aligned(&self) -> bool;
    fn align_up_to_page(&self) -> Self;
    fn align_down_to_page(&self) -> Self;
}
```

## 使用场景

### 场景 1：内核地址空间映射

```rust
extern "C" {
    fn stext();
    fn etext();
}

// 获取 .text 段地址范围
let text_start = Vaddr::new(stext as usize);
let text_end = Vaddr::new(etext as usize);

// 创建映射区域
space.push(MappingArea::new(
    VaddrRange::new(text_start, text_end),
    MapType::Direct,
    UniversalPTEFlag::kernel_r() | UniversalPTEFlag::X,
    AreaType::KernelText,
));
```

### 场景 2：物理帧分配器初始化

```rust
// 计算可用物理内存范围
let ekernel_paddr = unsafe { vaddr_to_paddr(ekernel as usize) };
let start = Ppn::from_addr_ceil(Paddr::new(ekernel_paddr));
let end = Ppn::from_addr_floor(Paddr::new(MEMORY_END));

// 初始化分配器
init_frame_allocator(start, end);
```

### 场景 3：拷贝数据到物理页

```rust
pub fn copy_data(&self, page_table: &ActivePageTableInner, data: &[u8]) {
    let mut offset = 0;
    for vpn in self.vpn_range {
        // 翻译虚拟地址到物理地址
        let paddr = page_table.translate(vpn.start_addr()).unwrap();

        // 转换为可访问的虚拟地址
        let vaddr = paddr.to_vaddr();

        // 拷贝数据
        let dst = unsafe {
            core::slice::from_raw_parts_mut(vaddr.as_usize() as *mut u8, PAGE_SIZE)
        };
        let len = core::cmp::min(PAGE_SIZE, data.len() - offset);
        dst[..len].copy_from_slice(&data[offset..offset + len]);
        offset += len;
    }
}
```

## 常见错误

### 错误 1：忘记 Range 是左闭右开

```rust
// ❌ 错误：期望包含结束地址
let range = VaddrRange::new(start_addr, end_addr);
assert!(range.contains(&end_addr));  // 断言失败！

// ✅ 正确：end 应为 end_addr.step()
let range = VaddrRange::new(start_addr, end_addr.step());
assert!(range.contains(&end_addr));  // 通过
```

### 错误 2：混用物理地址和虚拟地址

```rust
// ❌ 编译错误
let paddr = Paddr::new(0x8000_0000);
let vaddr: Vaddr = paddr;  // 类型不匹配！

// ✅ 正确：显式转换
let vaddr = paddr.to_vaddr();
```

### 错误 3：未检查对齐

```rust
// ❌ 页表根地址未对齐可能导致硬件异常
let root_ppn = Ppn::from_addr_floor(paddr);

// ✅ 应先检查对齐
assert!(paddr.is_page_aligned());
let root_ppn = Ppn::from_addr_floor(paddr);
```

### 错误 4：对齐值不是 2 的幂

```rust
// ❌ 错误：对齐值必须是 2 的幂
let addr = Vaddr::new(0x1234);
let aligned = addr.align_up(15);  // 15 不是 2 的幂

// ✅ 正确：使用 2 的幂作为对齐值
let aligned = addr.align_up(16);  // 16 = 2^4
```

**原因**：对齐算法 `(value + align - 1) & !(align - 1)` 仅对 2 的幂有效。

## 性能考量

### 零成本抽象

```rust
use core::mem::{size_of, align_of};

// 大小和对齐与 usize 相同
assert_eq!(size_of::<Paddr>(), size_of::<usize>());
assert_eq!(align_of::<Paddr>(), align_of::<usize>());
```

### 内联优化

关键方法标记为 `#[inline]`，在 `release` 模式下会被内联，编译为零成本的机器指令：

```rust
#[inline]
pub fn new(value: usize) -> Self { Self(value) }

#[inline]
pub fn as_usize(&self) -> usize { self.0 }

#[inline]
pub const fn align_up(&self, align: usize) -> Self { /* ... */ }
```

## 相关文档

- [整体架构](architecture.md) - MM 子系统分层设计
- [物理帧分配器](frame_allocator.md) - Ppn 的实际使用
- [页表抽象层](page_table.md) - Vpn/Ppn 的页表映射
- [API 参考](api_reference.md) - 快速 API 查询

## 参考实现

- **源代码**：`os/src/mm/address/`
- **架构接口**：`os/src/arch/*/mm/mod.rs`（地址转换函数）
