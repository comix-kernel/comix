# 抢占控制

## 简介

抢占控制（Preemption Control）是多核操作系统中的关键机制，用于防止任务在访问 Per-CPU 数据期间被调度器迁移到其他 CPU 核心。通过禁用抢占，可以确保任务在访问期间始终运行在同一个核心上，从而保证数据一致性。

在可抢占的内核中，调度器可以在任何时刻中断当前任务并切换到另一个任务。如果任务在访问 Per-CPU 变量期间被抢占并迁移到另一个核心，它会访问到错误核心的数据副本，导致数据不一致。抢占控制通过临时禁用调度器的抢占功能，确保任务在关键区域内不会被迁移。

## 核心概念

### 抢占与任务迁移

在多核系统中，调度器可能会：
1. **抢占当前任务**：中断正在运行的任务，切换到另一个任务
2. **任务迁移**：将任务从一个 CPU 核心迁移到另一个核心（负载均衡）

如果任务在访问 Per-CPU 数据期间被迁移：
```rust
// 假设任务最初在 CPU 0 上运行
let cpu_id = cpu_id();  // 返回 0
let data = per_cpu_data[cpu_id];  // 访问 CPU 0 的数据

// 此时任务被抢占并迁移到 CPU 1
// 但 cpu_id 仍然是 0，导致访问错误的数据副本
data.modify();  // 错误：修改了 CPU 0 的数据，但任务在 CPU 1 上运行
```

### 抢占计数器

抢占控制使用**嵌套计数器**机制：
- 每个 CPU 维护一个抢占计数器
- 计数器 > 0 表示抢占已禁用
- 支持嵌套调用：每次 `preempt_disable()` 增加计数器，每次 `preempt_enable()` 减少计数器
- 只有当计数器降为 0 时，抢占才真正启用

这种设计允许嵌套的临界区：
```rust
preempt_disable();  // 计数器: 0 -> 1
{
    preempt_disable();  // 计数器: 1 -> 2
    // 内层临界区
    preempt_enable();   // 计数器: 2 -> 1
}
// 外层临界区仍然受保护
preempt_enable();   // 计数器: 1 -> 0，抢占启用
```

## 数据结构

### 抢占计数器数组

```rust
use crate::config::MAX_CPU_COUNT;

/// 创建 Per-CPU 抢占计数器数组的辅助宏
macro_rules! create_preempt_count_array {
    ($size:expr) => {{
        const SIZE: usize = $size;
        const INIT: AtomicUsize = AtomicUsize::new(0);
        [INIT; SIZE]
    }};
}

static PREEMPT_COUNT: [AtomicUsize; MAX_CPU_COUNT] = create_preempt_count_array!(MAX_CPU_COUNT);
```

使用静态数组为每个 CPU 维护独立的抢占计数器。数组大小由 `MAX_CPU_COUNT` 配置常量决定（默认为 8），可在 `config.rs` 中修改以支持更多 CPU 核心。

**实现说明**：使用宏来生成数组初始化代码，确保数组元素数量与 `MAX_CPU_COUNT` 一致。由于 `AtomicUsize::new(0)` 是 `const fn`，可以在编译期完成所有初始化。

**源码位置**：`os/src/sync/preempt.rs:10`

## API 接口

### 禁用抢占

```rust
pub fn preempt_disable()
```

禁用当前 CPU 的抢占功能。可以嵌套调用，每次调用增加抢占计数器。

**实现细节**：
1. 获取当前 CPU ID
2. 原子地增加该 CPU 的抢占计数器
3. 插入 Acquire 内存屏障，确保后续访问不会被重排到此之前

**示例**：
```rust
use sync::preempt_disable;

preempt_disable();
// 临界区：访问 Per-CPU 数据
// 任务不会被迁移到其他核心
```

**源码位置**：`os/src/sync/preempt.rs:25`

### 启用抢占

```rust
pub fn preempt_enable()
```

启用当前 CPU 的抢占功能。必须与 `preempt_disable()` 配对使用。

**实现细节**：
1. 插入 Release 内存屏障，确保之前的访问不会被重排到此之后
2. 获取当前 CPU ID
3. 原子地减少该 CPU 的抢占计数器

**示例**：
```rust
use sync::{preempt_disable, preempt_enable};

preempt_disable();
// 临界区
preempt_enable();
```

**注意**：必须确保每个 `preempt_disable()` 都有对应的 `preempt_enable()`，否则抢占将永久禁用，导致系统无法调度。

**源码位置**：`os/src/sync/preempt.rs:36`

### 检查抢占状态

```rust
pub fn preempt_disabled() -> bool
```

检查当前 CPU 的抢占是否已禁用。

**返回值**：
- `true`：抢占已禁用（计数器 > 0）
- `false`：抢占已启用（计数器 = 0）

**示例**：
```rust
use sync::preempt_disabled;

if preempt_disabled() {
    println!("Preemption is disabled");
} else {
    println!("Preemption is enabled");
}
```

**源码位置**：`os/src/sync/preempt.rs:45`

### RAII 守卫

```rust
pub struct PreemptGuard;

impl PreemptGuard {
    pub fn new() -> Self
}
```

抢占保护的 RAII 守卫。创建时自动禁用抢占，销毁时自动启用抢占。

**优势**：
- 自动管理抢占状态，避免忘记调用 `preempt_enable()`
- 即使发生 panic，守卫的 `drop()` 也会被调用，确保抢占被正确恢复
- 代码更简洁，意图更清晰

**示例**：
```rust
use sync::PreemptGuard;

{
    let _guard = PreemptGuard::new();  // 抢占已禁用
    // 临界区：访问 Per-CPU 数据
}  // 守卫销毁，抢占自动启用
```

**源码位置**：`os/src/sync/preempt.rs:53`

## 使用场景

### 1. 访问 Per-CPU 变量

这是抢占控制最主要的用途：

```rust
use sync::{PerCpu, PreemptGuard};

static COUNTER: PerCpu<usize> = PerCpu::new(|| 0);

fn increment_counter() {
    let _guard = PreemptGuard::new();
    let counter = COUNTER.get_mut();
    *counter += 1;
}
```

### 2. 保护短临界区

对于不需要锁但需要保证原子性的短操作：

```rust
use sync::PreemptGuard;

fn update_local_state() {
    let _guard = PreemptGuard::new();
    // 更新当前 CPU 的本地状态
    // 确保操作不会被中断
}
```

### 3. 嵌套临界区

支持嵌套的抢占禁用：

```rust
use sync::PreemptGuard;

fn outer_function() {
    let _guard1 = PreemptGuard::new();  // 计数器: 0 -> 1
    // 外层临界区
    inner_function();
    // 外层临界区继续
}

fn inner_function() {
    let _guard2 = PreemptGuard::new();  // 计数器: 1 -> 2
    // 内层临界区
}  // 计数器: 2 -> 1
```

## 内存屏障

抢占控制使用内存屏障确保正确的内存顺序：

### Acquire 屏障（preempt_disable）

```rust
core::sync::atomic::fence(Ordering::Acquire);
```

在禁用抢占后插入 Acquire 屏障，确保：
- 后续的内存访问不会被重排到屏障之前
- 可以安全地读取 Per-CPU 数据

### Release 屏障（preempt_enable）

```rust
core::sync::atomic::fence(Ordering::Release);
```

在启用抢占前插入 Release 屏障，确保：
- 之前的内存访问不会被重排到屏障之后
- Per-CPU 数据的修改对其他核心可见

### 为什么需要屏障？

考虑以下场景：
```rust
preempt_disable();
let data = per_cpu_data.get_mut();
data.value = 42;  // 写入
preempt_enable();
```

如果没有内存屏障：
1. 编译器或 CPU 可能将 `data.value = 42` 重排到 `preempt_disable()` 之前
2. 此时任务可能被迁移，导致写入错误的数据副本

内存屏障防止了这种重排，确保所有 Per-CPU 访问都在抢占禁用期间完成。

## 设计考量

### 优势

1. **轻量级**：相比锁，抢占控制的开销极小（只是原子操作和内存屏障）
2. **嵌套支持**：通过计数器机制支持嵌套调用
3. **RAII 友好**：提供守卫类型，自动管理抢占状态

### 劣势

1. **实时性影响**：禁用抢占期间，高优先级任务无法抢占当前任务
2. **不保护中断**：抢占控制不影响中断处理，中断仍然可以打断当前任务
3. **单核无效**：在单核系统中，抢占控制无法防止中断处理程序的并发访问

### 与中断屏蔽的区别

| 特性 | 抢占控制 | 中断屏蔽 |
|------|---------|---------|
| 防止任务抢占 | ✓ | ✓ |
| 防止中断 | ✗ | ✓ |
| 开销 | 低 | 中等 |
| 实时性影响 | 中等 | 高 |
| 适用场景 | Per-CPU 数据访问 | 中断与任务共享数据 |

## 实现细节

### CPU ID 获取

抢占控制通过 `arch::kernel::cpu::cpu_id()` 获取当前 CPU 的 ID：

```rust
let cpu_id = crate::arch::kernel::cpu::cpu_id();
```

在 RISC-V 架构中，CPU ID 通常存储在 `tp`（Thread Pointer）寄存器中。

**注意**：当前实现暂时只支持单核，`cpu_id()` 始终返回 0。多核支持正在开发中。

### 原子操作

抢占计数器使用 `AtomicUsize` 和 `Relaxed` 内存顺序：

```rust
PREEMPT_COUNT[cpu_id].fetch_add(1, Ordering::Relaxed);
```

使用 `Relaxed` 是安全的，因为：
- 每个 CPU 只访问自己的计数器，不存在跨核竞争
- 内存顺序由显式的 `fence()` 保证

### 数组大小配置

抢占计数器使用固定大小的数组，大小由 `config::MAX_CPU_COUNT` 常量决定（默认为 8）。

**配置方法**：在 `os/src/config.rs` 中修改 `MAX_CPU_COUNT` 常量：

```rust
// 支持最多 16 个 CPU 核心
pub const MAX_CPU_COUNT: usize = 16;
```

**设计权衡**：
- 使用固定数组而非动态分配（`Vec`）是为了性能考虑
- 抢占控制是极高频访问的底层机制，固定数组提供零开销访问
- 如需支持更多核心，只需修改配置常量并重新编译

## 与调度器的集成

抢占控制需要与调度器紧密集成：

1. **调度器检查**：在调度点，调度器必须检查 `preempt_disabled()`
2. **延迟调度**：如果抢占已禁用，调度器应延迟调度，直到抢占启用
3. **抢占标志**：可以设置"需要调度"标志，在 `preempt_enable()` 时触发调度

```rust
pub fn preempt_enable() {
    core::sync::atomic::fence(Ordering::Release);
    let cpu_id = crate::arch::kernel::cpu::cpu_id();
    let count = PREEMPT_COUNT[cpu_id].fetch_sub(1, Ordering::Relaxed);

    // 如果计数器降为 0 且有待处理的调度请求
    if count == 1 && need_resched() {
        schedule();  // 触发调度
    }
}
```

**注意**：当前实现尚未完全集成调度器，这是未来的改进方向。

## 测试

抢占控制模块包含以下测试：

1. **基本功能测试**（`test_preempt_disable_enable`）：测试禁用和启用抢占
2. **守卫测试**（`test_preempt_guard`）：测试 RAII 守卫的自动管理
3. **嵌套守卫测试**（`test_nested_preempt_guard`）：测试嵌套的抢占禁用

运行测试：
```bash
cd os && make test
```

## 使用建议

### 最佳实践

1. **优先使用守卫**：使用 `PreemptGuard` 而不是手动调用 `preempt_disable/enable`
2. **保持临界区短小**：禁用抢占期间应尽快完成操作，避免影响实时性
3. **避免阻塞操作**：禁用抢占期间不应进行可能阻塞的操作（如 I/O）
4. **配对使用**：确保每个 `preempt_disable()` 都有对应的 `preempt_enable()`

### 常见陷阱

1. **忘记启用抢占**：
```rust
// 错误：忘记调用 preempt_enable()
preempt_disable();
// 临界区
// 缺少 preempt_enable()，抢占永久禁用
```

2. **在禁用抢占期间睡眠**：
```rust
// 错误：禁用抢占期间不应睡眠
preempt_disable();
sleep();  // 错误！可能导致死锁
preempt_enable();
```

3. **过长的临界区**：
```rust
// 不推荐：临界区过长，影响实时性
preempt_disable();
for i in 0..1000000 {
    // 大量计算
}
preempt_enable();
```

## 相关资源

- **源码位置**：`os/src/sync/preempt.rs`
- **Per-CPU 变量**：[Per-CPU 文档](./per_cpu.md)
- **同步机制概述**：[同步机制文档](./README.md)

## 未来改进

1. **调度器集成**：完善与调度器的集成，支持延迟调度
2. **动态核心数**：支持运行时动态调整核心数量
3. **抢占延迟统计**：记录抢占禁用的时长，用于性能分析
4. **调试支持**：在调试模式下检测过长的抢占禁用，帮助发现问题
