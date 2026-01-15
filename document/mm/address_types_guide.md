# 地址类型使用指南

## 概述

Comix 内核现在提供三种类型安全的地址类型，用于防止地址混淆和提高代码安全性：

- `PA` / `Paddr` - 物理地址（Physical Address）
- `VA` / `Vaddr` - 虚拟地址（Virtual Address）
- `UA` / `Uaddr` - 用户地址（User Address）

## 类型定义

所有地址类型都是 `#[repr(transparent)]` 的新类型封装，零运行时开销：

```rust
pub use Paddr as PA;    // 物理地址
pub use Vaddr as VA;    // 虚拟地址（内核或用户）
pub use Uaddr as UA;    // 用户地址（语义标记）
```

## 使用场景

### 1. PA（物理地址）

**用途**：表示物理内存地址

**典型场景**：
- 页帧分配
- DMA 操作
- 物理内存管理

```rust
use crate::mm::address::{PA, Paddr};

// 分配物理页帧
let ppn = alloc_frame()?;
let paddr: PA = PA::from_ppn(ppn);

// 转换为内核虚拟地址
let vaddr = paddr.to_vaddr();
```

### 2. VA（虚拟地址）

**用途**：表示内核虚拟地址

**典型场景**：
- 内核地址空间操作
- 设备映射
- 内核堆栈

```rust
use crate::mm::address::{VA, Vaddr};

// 内核栈指针
let kernel_stack_top: VA = VA::from_value(0x8_0400_0000);

// 从指针创建
let vaddr = VA::from_ref(&some_data);

// 转换为物理地址
let paddr = vaddr.to_paddr();
```

### 3. UA（用户地址）

**用途**：**语义标记**，表示用户进程地址空间的地址

**典型场景**：
- 系统调用参数
- 用户栈/堆操作
- 用户数据拷贝

```rust
use crate::mm::address::{UA, Uaddr, VA};

// 系统调用中的用户地址
unsafe fn sys_write(fd: usize, buf: UA, count: usize) -> isize {
    // 验证用户地址
    if !buf.is_valid_user_address() {
        return -EFAULT;
    }

    // 转换为 VA 进行操作
    let vaddr: VA = buf.to_vaddr();

    // ... 拷贝数据
}
```

## 转换规则

### VA ↔ PA 转换

```rust
let vaddr: VA = VA::from_value(0x8000_0000);
let paddr: PA = vaddr.to_paddr();      // 需要查询页表
let back: VA = paddr.to_vaddr();       // 通过线性映射
```

### VA ↔ UA 转换

```rust
let uaddr: UA = UA::from_value(0x4000_0000);
let vaddr: VA = uaddr.to_vaddr();      // 安全转换（仅语义区分）
let back: UA = unsafe { vaddr.to_uaddr() }; // 需要确保在用户地址空间
```

## 迁移指南

### 何时使用 UA

在以下情况使用 `UA` 而不是 `VA`：

1. **系统调用参数** - 来自用户空间的指针
   ```rust
   // 旧代码：
   fn sys_read(fd: usize, buf: usize, count: usize) -> isize

   // 新代码：
   fn sys_read(fd: usize, buf: UA, count: usize) -> isize
   ```

2. **用户栈/堆** - 明确标记为用户空间
   ```rust
   // 用户栈顶
   let user_stack_top: UA = UA::from_value(USER_STACK_TOP);
   ```

3. **用户数据拷贝** - 安全的用户空间访问
   ```rust
   unsafe fn copy_from_user(src: UA, dst: VA, len: usize) -> Result<()> {
       // 实现 ...
   }
   ```

### 何时保持使用 VA

以下情况继续使用 `VA`：

1. **内核地址空间** - 内核代码、数据、栈
2. **物理内存映射** - 直接映射区域
3. **MMIO 区域** - 设备寄存器

## 示例：用户空间字符串拷贝

```rust
use crate::mm::address::{UA, VA};

/// 安全地从用户空间拷贝以 null 结尾的字符串
pub unsafe fn copy_string_from_user(uaddr: UA, dst: &mut [u8]) -> Result<usize> {
    let mut len = 0;

    // 逐字节拷贝，检查 null 终止符
    while len < dst.len() {
        // UA -> VA 转换
        let src_va: VA = uaddr.add(len).to_vaddr();

        // 从用户空间读取一个字节
        let byte = src_va.as_ref::<u8>()?;

        dst[len] = byte;
        len += 1;

        if byte == 0 {
            break;
        }
    }

    Ok(len)
}
```

## 类型安全保证

### 编译期检查

```rust
// ✅ 正确：类型明确区分
fn map_page(paddr: PA, vaddr: VA) { /* ... */ }

// ❌ 错误：编译器会捕获类型混淆
fn bad_example(paddr: PA, vaddr: VA) {
    let x: VA = paddr;  // 编译错误！类型不匹配
}
```

### 运行时检查（debug 模式）

```rust
#[cfg(debug_assertions)]
pub fn validate_address_type_compatibility() {
    assert_eq!(core::mem::size_of::<PA>(), core::mem::size_of::<usize>());
    assert_eq!(core::mem::size_of::<VA>(), core::mem::size_of::<usize>());
    assert_eq!(core::mem::size_of::<UA>(), core::mem::size_of::<usize>());
}
```

## 性能说明

- **零开销**：所有地址类型都是 `#[repr(transparent)]`
- **无运行时成本**：编译后等同于 `usize`
- **编译期优化**：类型检查在编译时完成

## 迁移策略

### 阶段 1：新代码使用新类型

```rust
// 新编写的代码优先使用 PA/VA/UA
fn new_function(addr: PA) -> PA { /* ... */ }
```

### 阶段 2：添加类型安全包装

```rust
// 保留旧函数，添加类型安全版本
pub fn vaddr_to_paddr_usize(vaddr: usize) -> usize {
    // 旧实现
}

pub fn vaddr_to_paddr(vaddr: VA) -> PA {
    PA::from_usize(vaddr_to_paddr_usize(vaddr.as_usize()))
}
```

### 阶段 3：渐进迁移

- 优先迁移热点代码
- 优先迁移安全关键代码
- 保持向后兼容

## 相关文档

- `REFACTOR_PLAN.md` - 重构计划
- `REFACTOR_LOG.md` - 重构日志
- `os/src/mm/address/address.rs` - 地址类型实现
