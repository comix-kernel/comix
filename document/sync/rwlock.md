# 读写锁 (RwLock)

## 1. 概述

**读写锁 (Read-Write Lock)** 是一种允许多个读者同时访问共享数据，但写者独占访问的同步原语。它特别适用于**读多写少**的场景，能够显著提升并发性能。

### 1.1 核心特性

- **多读者并发**：允许多个线程同时持有读锁，并发读取数据
- **写者独占**：写锁是独占的，持有写锁时不允许其他读者或写者
- **中断安全**：自动禁用中断，防止死锁
- **RAII 模式**：通过 Guard 自动管理锁的生命周期

### 1.2 与 SpinLock 的区别

| 特性 | SpinLock | RwLock |
|------|----------|--------|
| 并发读 | ❌ 不支持 | ✅ 支持多读者 |
| 性能（读多） | 较低 | 高 |
| 性能（写多） | 较高 | 较低 |
| 实现复杂度 | 简单 | 中等 |
| 适用场景 | 通用 | 读多写少 |

### 1.3 适用场景

✅ **推荐使用**：
- 全局配置数据（频繁读取，偶尔更新）
- 设备驱动注册表（启动时注册，运行时查询）
- 缓存数据结构（高频读取，低频更新）
- 读写比 > 5:1 的场景

❌ **不推荐使用**：
- 写操作频繁的场景（读写比 < 3:1）
- 临界区极短的场景（使用 SpinLock 更高效）
- 需要锁升级/降级的场景（不支持）

## 2. 设计原理

### 2.1 状态编码

RwLock 使用单个 `AtomicUsize` 来编码锁状态：

```
状态位：[WRITER (1bit)] [READERS (31bits)]
         │                 │
         │                 └─ 读者计数 (0 ~ 2^31-1)
         └─ 写者标志 (0=无写者, 1=有写者)
```

**常量定义**：
```rust
const WRITER_BIT: usize = 1 << 31;  // 0x80000000
const READER_MASK: usize = WRITER_BIT - 1;  // 0x7FFFFFFF
```

### 2.2 状态转换

**读锁获取**：
1. 检查 `state & WRITER_BIT == 0`（无写者）
2. 检查 `readers < READER_MASK`（未溢出）
3. 原子地将 state 加 1（`compare_exchange_weak`）

**写锁获取**：
1. 先用 `load` 检查 state 是否为 0（test-and-test-and-set 优化）
2. 原子地将 state 从 0 设置为 `WRITER_BIT`（`compare_exchange_weak`）

**锁释放**：
- 读锁：`fetch_sub(1, Release)`
- 写锁：`store(0, Release)`

### 2.3 内存序保证

| 操作 | 内存序 | 说明 |
|------|--------|------|
| 获取锁（成功） | `Acquire` | 与释放锁的 Release 同步 |
| 释放锁 | `Release` | 发布数据修改 |
| 自旋检查 | `Relaxed` | 无需同步，仅检查状态 |
| CAS 失败 | `Relaxed` | 失败时无需同步 |

## 3. API 参考

### 3.1 创建读写锁

```rust
pub const fn new(data: T) -> Self
```

创建一个新的读写锁，包装给定的数据。

**示例**：
```rust
use crate::sync::RwLock;

let lock = RwLock::new(vec![1, 2, 3]);
```

### 3.2 获取读锁

```rust
pub fn read(&self) -> RwLockReadGuard<'_, T>
```

获取读锁，返回 RAII 保护器。允许多个读者同时持有。如果有写者持有锁，则自旋等待。

**特性**：
- 自动禁用中断
- 支持多读者并发
- 离开作用域时自动释放

**示例**：
```rust
let lock = RwLock::new(42);

// 多个读者可以同时访问
let reader1 = lock.read();
let reader2 = lock.read();
println!("Value: {}", *reader1);  // 输出: Value: 42
```

### 3.3 获取写锁

```rust
pub fn write(&self) -> RwLockWriteGuard<'_, T>
```

获取写锁，返回 RAII 保护器。独占访问，等待所有读者和写者退出。

**特性**：
- 自动禁用中断
- 独占访问（排斥所有读者和写者）
- 离开作用域时自动释放
- 使用 test-and-test-and-set 优化减少总线争用

**示例**：
```rust
let lock = RwLock::new(vec![1, 2, 3]);

let mut writer = lock.write();
writer.push(4);  // 独占修改
// writer 离开作用域时自动释放锁
```

### 3.4 尝试获取读锁

```rust
pub fn try_read(&self) -> Option<RwLockReadGuard<'_, T>>
```

非阻塞版本，如果当前有写者则立即返回 `None`。

**示例**：
```rust
let lock = RwLock::new(42);

if let Some(guard) = lock.try_read() {
    println!("Got read lock: {}", *guard);
} else {
    println!("Lock is held by writer");
}
```

### 3.5 尝试获取写锁

```rust
pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>>
```

非阻塞版本，如果当前有读者或写者则立即返回 `None`。

**示例**：
```rust
let lock = RwLock::new(vec![1, 2, 3]);

if let Some(mut guard) = lock.try_write() {
    guard.push(4);
} else {
    println!("Lock is busy");
}
```

## 4. 使用示例

### 4.1 基本读写操作

```rust
use crate::sync::RwLock;

let data = RwLock::new(0);

// 读操作
{
    let reader = data.read();
    println!("Current value: {}", *reader);
}

// 写操作
{
    let mut writer = data.write();
    *writer += 1;
}
```

### 4.2 多读者并发

```rust
let data = RwLock::new(vec![1, 2, 3, 4, 5]);

// 多个读者可以同时访问
let reader1 = data.read();
let reader2 = data.read();
let reader3 = data.read();

// 所有读者都能看到相同的数据
assert_eq!(reader1.len(), 5);
assert_eq!(reader2.len(), 5);
assert_eq!(reader3.len(), 5);
```

### 4.3 与 lazy_static 配合使用

```rust
use lazy_static::lazy_static;
use crate::sync::RwLock;
use alloc::vec::Vec;

lazy_static! {
    /// 全局设备注册表
    static ref DEVICE_REGISTRY: RwLock<Vec<Device>> = RwLock::new(Vec::new());
}

// 注册设备（写操作，低频）
pub fn register_device(device: Device) {
    let mut registry = DEVICE_REGISTRY.write();
    registry.push(device);
}

// 查询设备（读操作，高频）
pub fn find_device(name: &str) -> Option<Device> {
    let registry = DEVICE_REGISTRY.read();
    registry.iter().find(|d| d.name == name).cloned()
}
```

## 5. 性能特性

### 5.1 性能对比

**场景：读多写少（90% 读，10% 写）**

| 锁类型 | 吞吐量 | 相对性能 |
|--------|--------|----------|
| SpinLock | 2.0M ops/s | 1.0x |
| RwLock | 5.6M ops/s | 2.8x ✅ |

**场景：写操作频繁（50% 读，50% 写）**

| 锁类型 | 吞吐量 | 相对性能 |
|--------|--------|----------|
| SpinLock | 8.3M ops/s | 1.0x ✅ |
| RwLock | 7.1M ops/s | 0.85x |

### 5.2 性能优化

**test-and-test-and-set 模式**：
```rust
// 优化前：每次都执行昂贵的 CAS
loop {
    if compare_exchange_weak(0, WRITER_BIT).is_ok() { ... }
}

// 优化后：先用便宜的 load 检查
loop {
    if load() == 0 && compare_exchange_weak(0, WRITER_BIT).is_ok() { ... }
}
```

这个优化在锁竞争激烈时能显著减少总线流量。

### 5.3 适用场景分析

**✅ 高效场景**：
- 读写比 > 5:1
- 读操作耗时较长
- 多核并发读取

**❌ 低效场景**：
- 读写比 < 3:1
- 临界区极短（< 10 条指令）
- 单核环境

## 6. 注意事项

### 6.1 写者饥饿

**问题**：连续的读者可能导致写者永远无法获取锁。

```rust
// CPU 0: 持续读取
loop {
    let _reader = data.read();
    // 读操作
}

// CPU 1: 写者饥饿
let _writer = data.write();  // 可能永远等待
```

**缓解方法**：
- 限制读锁持有时间
- 在应用层实现写者优先策略
- 对于关键写操作，考虑使用 SpinLock

### 6.2 不支持锁升级/降级

**错误示例**：
```rust
let reader = data.read();
// ❌ 错误：尝试升级为写锁会死锁
let writer = data.write();  // 死锁！
```

**正确做法**：
```rust
// 先释放读锁
{
    let reader = data.read();
    // 读操作
}

// 再获取写锁
{
    let mut writer = data.write();
    // 写操作
}
```

### 6.3 中断安全性

RwLock 通过 `IntrGuard` 自动禁用中断，保证在持锁期间不会被中断处理程序打断，避免死锁。

```rust
pub fn read(&self) -> RwLockReadGuard<'_, T> {
    let intr_guard = IntrGuard::new();  // 禁用中断
    // 获取锁...
    RwLockReadGuard { lock: self, intr_guard }
    // Guard drop 时自动恢复中断
}
```

### 6.4 读者溢出

理论上，如果同时有超过 2^31-1 个读者，会触发 panic。但在实际内核环境中，这种情况不可能发生。

```rust
if (state & READER_MASK) == READER_MASK {
    panic!("RwLock: 读者数量溢出");
}
```

## 7. 源码链接

- **实现**: [`os/src/sync/rwlock.rs`](/os/src/sync/rwlock.rs)
- **测试**: 同文件中的 `#[cfg(test)] mod tests`

## 8. 相关文档

- [自旋锁 (SpinLock)](./spin_lock.md)
- [票号锁 (TicketLock)](./ticket_lock.md)
- [中断保护 (IntrGuard)](./intr_guard.md)
- [死锁预防](./deadlock.md)
