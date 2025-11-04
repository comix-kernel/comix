# 全局堆分配器

## 概述

全局堆分配器为内核提供动态内存分配能力，支持 Rust 标准库的 `alloc` crate（Vec、Box、String 等）。Comix 使用 **talc** 作为全局分配器实现。

### 为什么选择 talc？

- **无锁设计**：单核环境下性能优秀
- **零依赖**：适合裸机环境
- **灵活配置**：支持多种分配策略
- **稳定性好**：经过充分测试

## 实现

### 全局分配器定义

```rust
use talc::{Talc, Span};

#[global_allocator]
static ALLOCATOR: Talck<spin::Mutex<()>, ClaimOnOom> = Talc::new(unsafe {
    ClaimOnOom::new(Span::empty())
}).lock();
```

### 初始化流程

```rust
// os/src/mm/global_allocator/global_allocator.rs:30-40
pub fn init_heap() {
    extern "C" {
        fn sheap();
        fn eheap();
    }

    let heap_start = sheap as usize;
    let heap_size = eheap as usize - heap_start;

    unsafe {
        ALLOCATOR
            .lock()
            .claim(Span::new(heap_start as *mut u8, heap_start + heap_size as *mut u8))
            .expect("Failed to initialize heap");
    }
}
```

**链接脚本定义**（`linker.ld`）：

```ld
sheap = .;
. = . + 16M;  // KERNEL_HEAP_SIZE = 16MB
eheap = .;
```

### 内存布局

```
内核虚拟地址空间:

0xFFFF_FFC0_8020_0000 ← 内核加载地址
        ↓
[.text]
[.rodata]
[.data]
[.bss]
        ↓
sheap ──────────┐
                │
     [Heap]     │ 16MB（KERNEL_HEAP_SIZE）
                │
eheap ──────────┘
        ↓
[物理内存直接映射区]
```

## 基本使用

### Vec 动态数组

```rust
use alloc::vec::Vec;

let mut v = Vec::new();
for i in 0..100 {
    v.push(i);
}
assert_eq!(v.len(), 100);
```

### Box 堆分配

```rust
use alloc::boxed::Box;

// 分配单个值
let b = Box::new(42);
assert_eq!(*b, 42);

// 分配数组
let arr = Box::new([0u8; 4096]);
```

### String 字符串

```rust
use alloc::string::String;

let mut s = String::from("Hello, ");
s.push_str("Comix!");
assert_eq!(s, "Hello, Comix!");
```

### BTreeMap 有序映射

```rust
use alloc::collections::BTreeMap;

let mut map = BTreeMap::new();
map.insert("key1", "value1");
map.insert("key2", "value2");

assert_eq!(map.get("key1"), Some(&"value1"));
```

## OOM 处理

当堆内存耗尽时，`alloc_error_handler` 会被调用：

```rust
// os/src/mm/global_allocator/global_allocator.rs:50-55
#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!(
        "Heap allocation failed: size = {}, align = {}",
        layout.size(),
        layout.align()
    );
}
```

## 常见错误

### 错误 1：未初始化就使用

```rust
// ❌ 错误：在 mm::init() 之前使用 alloc
pub fn rust_main() {
    let v = Vec::new();  // panic: heap not initialized!
    mm::init();
}

// ✅ 正确：先初始化
pub fn rust_main() {
    mm::init();
    let v = Vec::new();  // OK
}
```

### 错误 2：堆溢出

```rust
// 内核堆仅 16MB，注意避免过大分配
let huge_vec: Vec<u8> = Vec::with_capacity(32 * 1024 * 1024);  // OOM!
```

**建议**：
- 大块内存使用物理帧分配器
- 控制动态数据结构的增长
- 必要时增加 `KERNEL_HEAP_SIZE`

### 错误 3：忘记 no_std 环境

```rust
// ❌ 错误：std::vec 在 no_std 中不可用
use std::vec::Vec;  // 编译错误！

// ✅ 正确：使用 alloc::vec
extern crate alloc;
use alloc::vec::Vec;
```

## 调试技巧

### 追踪堆分配

使用静态计数器追踪分配/释放：

```rust
use core::sync::atomic::{AtomicUsize, Ordering};

static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

// 在分配器中
unsafe impl GlobalAlloc for MyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        // ...
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        ALLOC_COUNT.fetch_sub(1, Ordering::Relaxed);
        // ...
    }
}

// 检查内存泄漏
assert_eq!(ALLOC_COUNT.load(Ordering::Relaxed), 0, "Memory leak detected!");
```

## 性能考量

### talc 特点

- **分配策略**：First-fit with splitting
- **时间复杂度**：O(n)（n 为空闲块数量）
- **碎片管理**：自动合并相邻空闲块
- **锁开销**：使用 `spin::Mutex`，单核下性能优秀

### 优化建议

1. **批量分配**：使用 `Vec::with_capacity` 预分配
2. **对象池**：频繁分配/释放的对象考虑使用对象池
3. **栈优先**：小对象优先使用栈分配

## 相关文档

- [物理帧分配器](frame_allocator.md) - 大块内存分配
- [整体架构](architecture.md) - MM 子系统初始化流程
- [配置常量](../../os/src/config.rs) - KERNEL_HEAP_SIZE

## 参考资料

- **talc 文档**：https://docs.rs/talc
- **GlobalAlloc trait**：https://doc.rust-lang.org/core/alloc/trait.GlobalAlloc.html
- **源代码**：`os/src/mm/global_allocator/`
