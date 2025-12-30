# RISC-V 核间中断 (IPI)

本文档详细描述 Comix 内核在 RISC-V 架构上的核间中断（Inter-Processor Interrupt, IPI）实现，包括设计原理、API 接口、使用场景和性能考虑。

## 1. 概述

### 1.1 什么是 IPI

IPI（Inter-Processor Interrupt，核间中断）是多核系统中 CPU 之间进行通信的基本机制。通过 IPI，一个 CPU 可以向其他 CPU 发送中断信号，通知它们执行特定操作。

### 1.2 主要用途

Comix 内核的 IPI 实现支持以下功能：

- **任务调度唤醒**：通知目标 CPU 有新任务需要调度
- **TLB 刷新同步**：页表更新后通知其他 CPU 刷新 TLB（TLB Shootdown）
- **系统停机协调**：关机时停止所有 CPU

### 1.3 当前状态

- **实现方式**：基于 SBI（Supervisor Binary Interface）的软件中断
- **支持的 IPI 类型**：Reschedule、TlbFlush、Stop
- **性能优化**：支持批量发送，减少 SBI 调用次数
- **内存安全**：使用原子操作，中断上下文不进行内存分配

## 2. 架构设计

### 2.1 IPI 类型

IPI 类型使用位标志表示，支持组合多种类型：

```rust
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum IpiType {
    /// 重新调度（通知目标 CPU 有新任务）
    Reschedule = 1 << 0,  // 0b001
    /// TLB 刷新（页表更新后同步）
    TlbFlush = 1 << 1,    // 0b010
    /// 停止 CPU（系统关机）
    Stop = 1 << 2,        // 0b100
}
```

使用位标志的好处：
- 可以组合多种 IPI 类型（例如：`Reschedule | TlbFlush`）
- 使用原子操作的 `fetch_or` 可以高效地设置多个标志
- 接收端可以一次处理多种 IPI 类型

### 2.2 Per-CPU 待处理标志

每个 CPU 都有一个独立的原子变量，存储待处理的 IPI 类型：

```rust
/// Per-CPU 待处理 IPI 标志
static IPI_PENDING: [AtomicU32; MAX_CPU_COUNT] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    // ... 最多 8 个 CPU
];
```

**设计要点**：
- 使用 `AtomicU32` 保证多核环境下的线程安全
- 每个 CPU 独立的变量，避免缓存行竞争（False Sharing）
- 使用 `Release`/`AcqRel` 内存序保证可见性

### 2.3 工作流程

IPI 的完整工作流程如下：

```
发送端 CPU:
1. IPI_PENDING[target].fetch_or(ipi_type)  // 设置待处理标志
2. sbi::send_ipi(hart_mask)                // 通过 SBI 触发软件中断
    ↓
    ↓ (SBI 固件处理)
    ↓
接收端 CPU:
1. 触发 Trap::Interrupt(1) 软件中断
2. trap_handler 调用 handle_ipi()
3. 读取并清除 IPI_PENDING[cpu]
4. 根据标志位执行相应操作：
   - Reschedule: 标记需要重新调度
   - TlbFlush: 执行 sfence.vma 刷新 TLB
   - Stop: 进入 WFI 循环停机
```

## 3. 实现原理

### 3.1 基于 SBI 的 IPI 发送

RISC-V 平台通过 SBI（Supervisor Binary Interface）提供 IPI 支持。Comix 使用两种 SBI 接口：

1. **SBI IPI 扩展**（优先使用）：
   ```rust
   const EID_IPI: usize = 0x735049;
   const FID_SEND_IPI: usize = 0;
   sbi_call(EID_IPI, FID_SEND_IPI, hart_mask, 0, 0);
   ```

2. **Legacy SBI**（回退方案）：
   ```rust
   const LEGACY_SEND_IPI: usize = 4;
   sbi_call(LEGACY_SEND_IPI, 0, &hart_mask as *const _ as usize, 0, 0);
   ```

实现会先尝试 SBI IPI 扩展，如果失败则回退到 Legacy SBI，确保兼容性。

### 3.2 软件中断处理

IPI 通过 RISC-V 的软件中断（Supervisor Software Interrupt）实现：

1. **中断号**：`Trap::Interrupt(1)`（SUPERVISOR_SOFTWARE）
2. **中断使能**：在 trap 初始化时通过 `sie::set_ssoft()` 使能
3. **处理入口**：在 `trap_handler.rs` 的 `user_trap` 和 `kernel_trap` 中处理

```rust
// 在 trap_handler.rs 中
Trap::Interrupt(1) => {
    // 软件中断（IPI）
    crate::arch::ipi::handle_ipi();
}
```

### 3.3 原子操作和内存序

IPI 实现使用严格的内存序保证正确性：

- **发送端**：使用 `Ordering::Release` 确保标志位设置对接收端可见
  ```rust
  IPI_PENDING[target_cpu].fetch_or(ipi_type as u32, Ordering::Release);
  ```

- **接收端**：使用 `Ordering::AcqRel` 确保读取到最新值并清除
  ```rust
  let pending = IPI_PENDING[cpu].swap(0, Ordering::AcqRel);
  ```

这保证了即使在弱内存序的架构上，IPI 也能正确工作。

## 4. API 接口

### 4.1 send_ipi - 发送 IPI 到单个 CPU

```rust
pub fn send_ipi(target_cpu: usize, ipi_type: IpiType)
```

**功能**：向指定 CPU 发送 IPI。

**参数**：
- `target_cpu`: 目标 CPU ID（0 到 NUM_CPU-1）
- `ipi_type`: IPI 类型（Reschedule、TlbFlush 或 Stop）

**使用示例**：
```rust
use crate::arch::ipi::{send_ipi, IpiType};

// 通知 CPU 1 有新任务需要调度
send_ipi(1, IpiType::Reschedule);

// 通知 CPU 2 刷新 TLB
send_ipi(2, IpiType::TlbFlush);
```

**注意事项**：
- 如果 `target_cpu >= NUM_CPU`，会触发 panic
- 可以向当前 CPU 发送 IPI（会立即触发软件中断）

### 4.2 send_ipi_many - 批量发送 IPI

```rust
pub fn send_ipi_many(hart_mask: usize, ipi_type: IpiType)
```

**功能**：向多个 CPU 批量发送 IPI，只需一次 SBI 调用。

**参数**：
- `hart_mask`: hart 位掩码，第 i 位为 1 表示向 CPU i 发送
- `ipi_type`: IPI 类型

**使用示例**：
```rust
// 向 CPU 1, 2, 3 发送调度 IPI
let mask = (1 << 1) | (1 << 2) | (1 << 3);  // 0b1110
send_ipi_many(mask, IpiType::Reschedule);

// 向所有 CPU 发送 TLB 刷新 IPI
let all_mask = (1 << NUM_CPU) - 1;
send_ipi_many(all_mask, IpiType::TlbFlush);
```

**性能优势**：
- 批量发送只需一次 SBI 调用，减少系统调用开销
- 适合需要通知多个 CPU 的场景（如 TLB Shootdown）

### 4.3 send_reschedule_ipi - 发送调度 IPI

```rust
pub fn send_reschedule_ipi(cpu: usize)
```

**功能**：通知目标 CPU 有新任务需要调度（`send_ipi` 的便捷封装）。

**参数**：
- `cpu`: 目标 CPU ID

**使用示例**：
```rust
// 唤醒 CPU 2 进行任务调度
send_reschedule_ipi(2);
```

**典型场景**：
- 任务迁移：将任务从一个 CPU 迁移到另一个 CPU
- 负载均衡：唤醒空闲 CPU 处理新任务
- 优先级抢占：高优先级任务到达时唤醒目标 CPU

### 4.4 send_tlb_flush_ipi_all - 广播 TLB 刷新

```rust
pub fn send_tlb_flush_ipi_all()
```

**功能**：向所有其他 CPU 广播 TLB 刷新 IPI（不包括当前 CPU）。

**使用示例**：
```rust
// 修改页表后，通知所有其他 CPU 刷新 TLB
unsafe {
    // 修改页表映射
    page_table.map(vaddr, paddr, flags);

    // 刷新当前 CPU 的 TLB
    core::arch::asm!("sfence.vma");

    // 通知其他 CPU 刷新 TLB
    send_tlb_flush_ipi_all();
}
```

**注意事项**：
- 当前 CPU 不会收到 IPI，需要自己刷新 TLB
- 这是一个同步点：调用后应等待其他 CPU 完成刷新（当前实现是异步的）

### 4.5 handle_ipi - 处理 IPI

```rust
pub fn handle_ipi()
```

**功能**：处理当前 CPU 收到的 IPI（在软件中断处理程序中调用）。

**处理流程**：
1. 读取并清除 `IPI_PENDING[cpu]`
2. 根据标志位执行相应操作：
   - `Reschedule`: 标记需要重新调度（实际调度在中断返回时进行）
   - `TlbFlush`: 执行 `sfence.vma` 刷新 TLB
   - `Stop`: 进入 WFI 循环停机

**注意事项**：
- 此函数在中断上下文中调用，不能进行内存分配或持有睡眠锁
- 调度 IPI 不会立即切换任务，而是在中断返回时由调度器处理

## 5. 使用场景

### 5.1 跨核任务唤醒

当一个 CPU 创建新任务或任务变为就绪状态时，可以通过 IPI 唤醒目标 CPU：

```rust
// 在 CPU 0 上创建任务，分配给 CPU 1
let task = Task::new(entry, args);
scheduler.add_task(task, target_cpu = 1);

// 唤醒 CPU 1 进行调度
send_reschedule_ipi(1);
```

### 5.2 TLB Shootdown

在多核系统中，修改页表后需要通知所有 CPU 刷新 TLB。Comix 提供了两种方式：

#### 自动 TLB Shootdown（推荐）

**从 SMP 分支开始，页表操作会自动处理 TLB shootdown**，无需手动调用：

```rust
// 页表操作会自动刷新所有 CPU 的 TLB
page_table.map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::user_rw())?;
// ✓ 自动刷新当前 CPU 的 TLB
// ✓ 自动通过 IPI 通知其他 CPU 刷新 TLB

page_table.unmap(vpn)?;
// ✓ 自动处理 TLB shootdown

page_table.update_flags(vpn, UniversalPTEFlag::kernel_rw())?;
// ✓ 自动处理 TLB shootdown
```

详见 [5.2.1 页表自动 TLB Shootdown](#521-页表自动-tlb-shootdown)。

#### 手动 TLB Shootdown（特殊场景）

在某些特殊场景下（如直接操作页表项、批量修改等），可能需要手动触发 TLB shootdown：

```rust
// 手动修改页表项后
unsafe {
    // 刷新当前 CPU 的 TLB
    core::arch::asm!("sfence.vma");

    // 通知所有其他 CPU 刷新 TLB
    send_tlb_flush_ipi_all();
}
```

**注意事项**：
- 大多数情况下应使用页表的标准 API（`map`/`unmap`/`update_flags`），它们会自动处理 TLB shootdown
- 只有在绕过页表 API 直接操作硬件时才需要手动调用
- 手动调用时必须先刷新当前 CPU 的 TLB，再发送 IPI

#### 5.2.1 页表自动 TLB Shootdown

Comix 的页表实现在 `PageTableInner` 中集成了自动 TLB shootdown 机制，确保多核环境下的内存一致性。

**实现原理**：

页表操作（`map`、`unmap`、`update_flags`）内部调用 `tlb_flush_all_cpus()` 方法：

```rust
impl PageTableInner {
    fn tlb_flush_all_cpus(vpn: Vpn) {
        // 1. 刷新当前 CPU 的 TLB
        <Self as PageTableInnerTrait<PageTableEntry>>::tlb_flush(vpn);

        // 2. 通知所有其他 CPU 刷新 TLB
        let num_cpu = unsafe { crate::kernel::NUM_CPU };
        if num_cpu > 1 {
            send_tlb_flush_ipi_all();
        }
    }
}
```

**行为特性**：

| 环境 | 行为 | 性能开销 |
|------|------|----------|
| 单核（NUM_CPU = 1） | 只刷新本地 TLB | 最小（~10 周期） |
| 多核（NUM_CPU > 1） | 刷新本地 TLB + 发送 IPI | 中等（~500 周期） |
| 测试模式 | 自动检测环境 | 根据环境决定 |

**使用示例**：

```rust
// 示例 1：映射用户页面
let vpn = Vpn::from_usize(0x10000);
let ppn = alloc_frame().unwrap().ppn();
page_table.map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::user_rw())?;
// ✓ 所有 CPU 的 TLB 已自动刷新

// 示例 2：批量映射
for i in 0..100 {
    let vpn = Vpn::from_usize(0x10000 + i * 0x1000);
    let ppn = alloc_frame().unwrap().ppn();
    page_table.map(vpn, ppn, PageSize::Size4K, UniversalPTEFlag::user_rw())?;
    // 每次映射都会触发 TLB shootdown
}
// 注意：批量操作可能产生较多 IPI，未来可优化为批量刷新

// 示例 3：修改权限
page_table.update_flags(vpn, UniversalPTEFlag::kernel_r())?;
// ✓ 权限更新后，所有 CPU 的 TLB 已自动刷新
```

**性能考虑**：

- **单核优化**：单核环境下无 IPI 开销，性能与传统实现相同
- **多核开销**：每次页表操作约增加 0.5 微秒（4 核系统）
- **批量操作**：频繁的页表修改会产生大量 IPI，建议：
  - 使用更大的页面（2MB/1GB）减少映射次数
  - 预分配页表，减少运行时修改
  - 未来可实现延迟批量刷新机制

**相关实现**：

- 源码位置：`os/src/arch/riscv/mm/page_table.rs`
- IPI 发送：`os/src/arch/riscv/ipi.rs` 中的 `send_tlb_flush_ipi_all()`
- 详细说明：参见 [页表文档](../../mm/page_table.md#多核-tlb-shootdown)

### 5.3 系统关机

关机时需要停止所有 CPU：

```rust
// 向所有其他 CPU 发送停止 IPI
let current = cpu_id();
let num_cpu = unsafe { crate::kernel::NUM_CPU };

for cpu in 0..num_cpu {
    if cpu != current {
        send_ipi(cpu, IpiType::Stop);
    }
}

// 等待所有 CPU 停止
// ...

// 当前 CPU 最后关机
sbi::shutdown(false);
```

## 6. 性能考虑

### 6.1 IPI 延迟

IPI 的延迟主要来自：

1. **SBI 调用开销**：从 S 模式陷入 M 模式（约 100-200 周期）
2. **中断传播延迟**：硬件传播中断信号（约 10-50 周期）
3. **中断处理开销**：保存上下文、调用处理函数（约 50-100 周期）

总延迟约为 **200-400 个时钟周期**（在 1GHz CPU 上约 0.2-0.4 微秒）。

### 6.2 优化策略

#### 6.2.1 批量发送

使用 `send_ipi_many` 代替多次 `send_ipi`：

```rust
// 不推荐：多次 SBI 调用
for cpu in 1..num_cpu {
    send_ipi(cpu, IpiType::TlbFlush);
}

// 推荐：一次 SBI 调用
let mask = ((1 << num_cpu) - 1) & !1;  // 除了 CPU 0
send_ipi_many(mask, IpiType::TlbFlush);
```

#### 6.2.2 合并 IPI 类型

利用位标志合并多种 IPI 类型：

```rust
// 同时发送调度和 TLB 刷新 IPI
let combined = (IpiType::Reschedule as u32) | (IpiType::TlbFlush as u32);
IPI_PENDING[target].fetch_or(combined, Ordering::Release);
sbi::send_ipi(1 << target);
```

#### 6.2.3 避免不必要的 IPI

- **本地操作**：如果目标是当前 CPU，直接执行操作而不发送 IPI
- **延迟批处理**：收集多个 IPI 请求，批量发送
- **TLB 优化**：使用 ASID（地址空间标识符）减少 TLB 刷新需求

### 6.3 性能开销

在典型的多核系统中，IPI 的性能开销：

- **单次 IPI**：约 0.5 微秒（包括发送和处理）
- **TLB Shootdown**：约 1-2 微秒（4 核系统）
- **任务唤醒**：约 0.3 微秒（仅发送 IPI）

对于大多数应用，IPI 开销可以忽略不计。但在高频场景（如频繁的页表修改）中，应考虑优化。

## 7. 调试和验证

### 7.1 检查中断使能

确保软件中断已使能：

```rust
// 在 trap::init() 中
unsafe {
    crate::arch::intr::enable_software_interrupt();
}

// 检查 sie 寄存器
let sie = riscv::register::sie::read();
assert!(sie.ssoft(), "Software interrupt not enabled");
```

### 7.2 测试 IPI 发送和接收

```rust
// 测试 IPI 类型标志
test_case!(test_ipi_type_flags, {
    kassert!(IpiType::Reschedule as u32 == 1);
    kassert!(IpiType::TlbFlush as u32 == 2);
    kassert!(IpiType::Stop as u32 == 4);
});

// 测试 IPI 类型组合
test_case!(test_ipi_type_combination, {
    let combined = (IpiType::Reschedule as u32) | (IpiType::TlbFlush as u32);
    kassert!(combined == 3);
});
```

### 7.3 常见问题排查

| 问题 | 可能原因 | 解决方法 |
|------|----------|----------|
| IPI 未触发 | 软件中断未使能 | 检查 `sie.ssoft` 位 |
| IPI 丢失 | 标志位被覆盖 | 使用 `fetch_or` 而非直接赋值 |
| 死锁 | 在 IPI 处理中持有锁 | 避免在 `handle_ipi` 中持有锁 |
| 性能下降 | 过多的 IPI | 使用批量发送，减少 IPI 频率 |

### 7.4 调试日志

启用 IPI 调试日志：

```rust
// 在 handle_ipi() 中
crate::pr_debug!("[IPI] CPU {} handling IPI: {:#x}", cpu, pending);
```

查看日志输出：
```
[IPI] CPU 1 handling IPI: 0x1  // Reschedule
[IPI] CPU 2 handling IPI: 0x2  // TlbFlush
[IPI] CPU 3 handling IPI: 0x3  // Reschedule | TlbFlush
```

## 8. 相关文件和参考资料

### 8.1 源码位置

| 文件路径 | 功能描述 |
|----------|----------|
| [`os/src/arch/riscv/ipi.rs`](/os/src/arch/riscv/ipi.rs) | IPI 核心实现 |
| [`os/src/arch/riscv/lib/sbi.rs`](/os/src/arch/riscv/lib/sbi.rs) | SBI IPI 接口封装 |
| [`os/src/arch/riscv/trap/trap_handler.rs`](/os/src/arch/riscv/trap/trap_handler.rs) | 软件中断处理入口 |
| [`os/src/arch/riscv/intr/mod.rs`](/os/src/arch/riscv/intr/mod.rs) | 软件中断使能/禁用 |
| [`os/src/arch/riscv/constant.rs`](/os/src/arch/riscv/constant.rs) | SUPERVISOR_SOFTWARE 常量 |

### 8.2 相关文档

- [多核启动](./smp_boot.md) - SMP 系统的启动流程和 Per-CPU 数据结构
- [SMP 与中断](../../sync/smp_interrupts.md) - SMP 系统中的中断和并发问题
- [Per-CPU 变量](../../sync/per_cpu.md) - Per-CPU 数据结构的详细说明

### 8.3 外部参考资料

- [RISC-V SBI Specification](https://github.com/riscv-non-isa/riscv-sbi-doc) - SBI IPI 扩展规范
- [RISC-V Privileged Specification](https://riscv.org/technical/specifications/) - 软件中断机制
- [Linux Kernel IPI Implementation](https://www.kernel.org/doc/html/latest/core-api/irq/irq-domain.html) - Linux 的 IPI 实现参考
