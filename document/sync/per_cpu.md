# Per-CPU 变量机制

## 简介

Per-CPU 变量机制允许每个 CPU 核心维护独立的数据副本，从而避免多核环境下的锁竞争。这是一种重要的无锁并发优化技术，特别适用于频繁访问的共享数据。

在多核系统中，如果多个 CPU 核心频繁访问同一个共享变量，即使使用自旋锁保护，也会因为缓存一致性协议（如 MESI）导致严重的性能开销。Per-CPU 变量通过为每个核心分配独立的数据副本，使得每个核心只访问自己的副本，从而完全消除了锁竞争和缓存行抖动（Cache Line Bouncing）。

## 核心概念

### 数据隔离

Per-CPU 变量的核心思想是**数据隔离**：
- 每个 CPU 核心拥有独立的数据副本
- 核心只访问自己的副本，不与其他核心共享
- 避免了缓存一致性开销和锁竞争

### 抢占保护

访问 Per-CPU 变量时必须禁用抢占，原因是：
- 如果任务在访问 Per-CPU 变量期间被抢占并迁移到另一个核心
- 它会访问到错误核心的数据副本，导致数据不一致
- 通过禁用抢占，确保任务在访问期间不会被迁移

## 数据结构

### PerCpu<T>

```rust
pub struct PerCpu<T> {
    data: Vec<CacheAligned<T>>,
}
```

`PerCpu<T>` 是 Per-CPU 变量的容器，内部维护一个 `Vec`，每个元素对应一个 CPU 核心的数据副本。

**缓存行对齐优化**：每个数据副本使用 `CacheAligned<T>` 包装，确保独占一个缓存行（64 字节）。这避免了伪共享（False Sharing）问题——当多个 CPU 核心修改位于同一缓存行内的不同数据时，会导致缓存行频繁失效和同步，严重影响性能。通过缓存行对齐，每个核心的数据副本互不干扰，充分发挥 Per-CPU 变量的性能优势。

**源码位置**：`os/src/sync/per_cpu.rs:32`

## API 接口

### 创建 Per-CPU 变量

```rust
pub fn new<F: Fn() -> T>(init: F) -> Self
```

创建一个 Per-CPU 变量，为每个 CPU 核心调用初始化函数 `init` 创建独立的数据副本。

**参数**：
- `init`: 初始化函数，为每个 CPU 创建一个数据副本

**Panics**：
- 如果 `NUM_CPU` 未设置或为 0，会 panic

**示例**：
```rust
use sync::PerCpu;
use core::sync::atomic::AtomicUsize;

// 为每个 CPU 创建一个独立的计数器
let counter = PerCpu::new(|| AtomicUsize::new(0));
```

**源码位置**：`os/src/sync/per_cpu.rs:44`

### 获取当前 CPU 的数据（只读）

```rust
pub fn get(&self) -> &T
```

获取当前 CPU 核心的数据副本的只读引用。

**Safety 要求**：
1. 当前 CPU ID 必须有效（< NUM_CPU）
2. 访问期间抢占必须已禁用（防止任务迁移）

**示例**：
```rust
use sync::{PerCpu, preempt_disable, preempt_enable};

let counter = PerCpu::new(|| 0usize);

preempt_disable();
let value = counter.get();
println!("Current CPU counter: {}", value);
preempt_enable();
```

**源码位置**：`os/src/sync/per_cpu.rs:63`

### 获取当前 CPU 的数据（可变）

```rust
pub fn get_mut(&self) -> &mut T
```

获取当前 CPU 核心的数据副本的可变引用。

**Safety 要求**：
1. 当前 CPU ID 必须有效
2. 访问期间抢占必须已禁用
3. 没有其他引用指向同一数据

**设计说明**：

此方法从 `&self` 返回 `&mut T`，这是 Per-CPU 变量的标准实现模式：
- Per-CPU 变量通常作为全局 `static` 使用，只能通过 `&self` 访问
- 每个 CPU 访问不同的数据副本，通过抢占控制保证独占访问
- 使用 `UnsafeCell` 提供内部可变性，类似于 `RefCell` 或 `Mutex`

这种设计允许 Per-CPU 变量作为静态全局变量使用，同时保持无锁的高性能特性。

**示例**：
```rust
use sync::{PerCpu, preempt_disable, preempt_enable};

let counter = PerCpu::new(|| 0usize);

preempt_disable();
let value = counter.get_mut();
*value += 1;
preempt_enable();
```

**源码位置**：`os/src/sync/per_cpu.rs:86`

### 获取指定 CPU 的数据（只读）

```rust
pub fn get_of(&self, cpu_id: usize) -> &T
```

获取指定 CPU 核心的数据副本的只读引用。用于跨核访问，例如负载均衡时查看其他 CPU 的队列长度。

**参数**：
- `cpu_id`: 目标 CPU 的 ID

**Panics**：
- 如果 `cpu_id` 无效（>= NUM_CPU），会 panic

**示例**：
```rust
use sync::PerCpu;

let counter = PerCpu::new(|| 0usize);

// 查看 CPU 0 的计数器值
let value = counter.get_of(0);
println!("CPU 0 counter: {}", value);
```

**源码位置**：`os/src/sync/per_cpu.rs:96`

## 使用场景

### 1. 统计计数器

Per-CPU 变量最常见的用途是实现高性能的统计计数器：

```rust
use sync::PerCpu;
use core::sync::atomic::{AtomicUsize, Ordering};

// 每个 CPU 维护独立的中断计数器
static INTERRUPT_COUNT: PerCpu<AtomicUsize> = PerCpu::new(|| AtomicUsize::new(0));

fn handle_interrupt() {
    // 中断处理程序中，抢占已自动禁用
    INTERRUPT_COUNT.get().fetch_add(1, Ordering::Relaxed);
}

fn get_total_interrupts() -> usize {
    let num_cpu = unsafe { crate::kernel::NUM_CPU };
    let mut total = 0;
    for cpu_id in 0..num_cpu {
        total += INTERRUPT_COUNT.get_of(cpu_id).load(Ordering::Relaxed);
    }
    total
}
```

### 2. Per-CPU 运行队列

调度器可以为每个 CPU 维护独立的运行队列，避免锁竞争：

```rust
use sync::PerCpu;
use alloc::collections::VecDeque;

struct RunQueue {
    tasks: VecDeque<TaskRef>,
}

static RUN_QUEUES: PerCpu<SpinLock<RunQueue>> = PerCpu::new(|| {
    SpinLock::new(RunQueue { tasks: VecDeque::new() })
});

fn enqueue_task(task: TaskRef) {
    preempt_disable();
    let queue = RUN_QUEUES.get().lock();
    queue.tasks.push_back(task);
    preempt_enable();
}
```

### 3. Per-CPU 缓存

为每个 CPU 维护独立的对象缓存，减少分配器竞争：

```rust
use sync::PerCpu;

struct ObjectCache<T> {
    free_list: Vec<T>,
}

static CACHE: PerCpu<ObjectCache<MyObject>> = PerCpu::new(|| {
    ObjectCache { free_list: Vec::new() }
});

fn alloc_object() -> Option<MyObject> {
    preempt_disable();
    let cache = CACHE.get_mut();
    let obj = cache.free_list.pop();
    preempt_enable();
    obj
}
```

## 设计考量

### 优势

1. **零锁开销**：完全避免锁竞争，每个核心独立访问自己的数据
2. **缓存友好**：数据副本通常位于核心的本地缓存中，访问延迟低
3. **可扩展性**：性能随核心数线性扩展，不会因核心增加而退化

### 劣势

1. **内存开销**：每个核心都有独立副本，内存使用量是单副本的 N 倍（N 为核心数）
2. **聚合成本**：如果需要获取全局视图（如总计数），需要遍历所有核心的副本
3. **抢占限制**：访问期间必须禁用抢占，可能影响实时性

### 适用场景

Per-CPU 变量适用于：
- 频繁写入的统计计数器
- 每个核心独立维护的数据结构（如运行队列）
- 需要高并发性能的场景

不适用于：
- 需要频繁聚合的数据
- 内存受限的环境
- 数据副本很大的情况

## 线程安全性

`PerCpu<T>` 实现了 `Send` 和 `Sync` trait：

```rust
unsafe impl<T: Send> Send for PerCpu<T> {}
unsafe impl<T: Send> Sync for PerCpu<T> {}
```

这是安全的，因为：
- 每个 CPU 访问不同的数据副本，不存在数据竞争
- 通过抢占控制保证任务不会在访问期间迁移
- 跨核访问（`get_of`）只提供只读引用，不会产生竞争

## 与抢占控制的配合

Per-CPU 变量必须与抢占控制配合使用：

```rust
use sync::{PerCpu, preempt_disable, preempt_enable};

let data = PerCpu::new(|| 0);

// 正确用法：禁用抢占
preempt_disable();
let value = data.get_mut();
*value += 1;
preempt_enable();

// 错误用法：未禁用抢占（可能导致数据不一致）
// let value = data.get_mut();  // 危险！
// *value += 1;
```

更推荐使用 RAII 守卫：

```rust
use sync::{PerCpu, PreemptGuard};

let data = PerCpu::new(|| 0);

{
    let _guard = PreemptGuard::new();  // 自动禁用抢占
    let value = data.get_mut();
    *value += 1;
}  // 守卫销毁时自动启用抢占
```

## 实现细节

### CPU ID 获取

Per-CPU 变量通过 `arch::kernel::cpu::cpu_id()` 获取当前 CPU 的 ID：

```rust
let cpu_id = crate::arch::kernel::cpu::cpu_id();
```

在 RISC-V 架构中，CPU ID 通常存储在 `tp`（Thread Pointer）寄存器中，可以快速读取。

**注意**：当前实现暂时只支持单核，`cpu_id()` 始终返回 0。多核支持正在开发中。

### 内存布局

`PerCpu<T>` 使用 `Vec<CacheAligned<T>>` 存储数据副本：
- 每个元素对应一个 CPU 核心
- 索引即为 CPU ID
- 使用 `CacheAligned<T>` 包装，确保每个数据副本独占一个缓存行（64 字节对齐）
- 内部使用 `UnsafeCell` 提供内部可变性

**缓存行对齐的重要性**：在多核系统中，如果多个 CPU 的数据位于同一缓存行，即使访问不同的数据，也会因为缓存一致性协议导致性能下降。通过确保每个 Per-CPU 数据独占一个缓存行，可以完全避免这种伪共享问题。

### 初始化时机

Per-CPU 变量在创建时需要知道 CPU 核心数量（`NUM_CPU`）：
- `NUM_CPU` 在内核启动早期由引导代码设置
- 如果在 `NUM_CPU` 设置前创建 Per-CPU 变量，会 panic

## 测试

Per-CPU 模块包含以下测试：

1. **基本功能测试**（`test_per_cpu_basic`）：测试创建和访问 Per-CPU 变量
2. **跨核访问测试**（`test_per_cpu_get_of`）：测试 `get_of` 方法
3. **可变访问测试**（`test_per_cpu_get_mut`）：测试 `get_mut` 方法

运行测试：
```bash
cd os && make test
```

## 相关资源

- **源码位置**：`os/src/sync/per_cpu.rs`
- **抢占控制**：[抢占控制文档](./preempt.md)
- **同步机制概述**：[同步机制文档](./README.md)

## 未来改进

1. **NUMA 感知**：在 NUMA 架构中，优化数据副本的内存分配位置
2. **动态核心数**：支持运行时动态调整核心数量
3. **统计聚合优化**：提供高效的聚合接口，减少遍历开销
