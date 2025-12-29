# RISC-V 多核启动 (SMP Boot)

本文档详细描述 Comix 内核在 RISC-V 架构上的多核启动实现，包括启动流程、Per-CPU 数据结构、tp 寄存器处理以及当前的限制。

## 1. 概述

### 1.1 目标

Comix 内核实现了基础的对称多处理（SMP）启动支持，能够在 QEMU RISC-V virt 平台上启动多个 CPU 核心。当前实现的主要目标是：

- 使用 SBI HSM (Hart State Management) 接口启动从核心
- 建立 Per-CPU 数据结构，为每个核心提供独立的运行环境
- 实现 tp 寄存器的保存与恢复（为内核 Per-CPU 访问和未来的用户态 TLS 做准备）
- 为未来的多核调度器奠定基础

### 1.2 当前状态

- **支持核心数**：最多 8 个核心（由 `config.rs` 中的 `MAX_CPU_COUNT` 定义）
- **主核心**：hartid 0 负责所有初始化工作和任务调度
- **从核心**：hartid 1-7 启动后进入 WFI 空挂状态，等待未来的多核调度器实现
- **调度状态**：当前仅主核心运行任务，从核心暂不参与调度

## 2. 启动流程

### 2.1 主核心启动流程

主核心（hartid 0）的启动流程如下：

```
entry.S (_start)
    ↓
清空 BSS 段
    ↓
boot/mod.rs (main)
    ↓
初始化内存管理 (mm_init)
    ↓
初始化 CPUS 数组，设置 tp 指向 CPU 0
    ↓
激活内核地址空间 (activate_kernel_space)
    ↓
初始化 trap、平台、时间
    ↓
启动从核心 (boot_secondary_cpus)
    ↓
初始化定时器
    ↓
启动第一个任务 (rest_init)
```

关键代码位于 [`os/src/arch/riscv/boot/mod.rs`](/os/src/arch/riscv/boot/mod.rs) 的 `main()` 函数（第 211-258 行）：

```rust
pub fn main(hartid: usize) {
    // 1. 清空 BSS
    clear_bss();

    // 2. 初始化内存管理
    let kernel_space = mm::init();

    // 3. 初始化 CPUS 并设置 tp 指向 CPU 0
    {
        use crate::kernel::CPUS;
        let cpu_ptr = &*CPUS.get_of(0) as *const _ as usize;
        unsafe {
            core::arch::asm!("mv tp, {}", in(reg) cpu_ptr);
        }
    }

    // 4. 激活内核地址空间并设置 current_memory_space
    {
        let _guard = crate::sync::PreemptGuard::new();
        current_cpu().switch_space(kernel_space);
    }

    // 5. 初始化 trap、平台、时间
    trap::init_boot_trap();
    platform::init();
    time::init();

    // 6. 启动从核心（在启用定时器中断之前）
    let num_cpus = unsafe { NUM_CPU };
    if num_cpus > 1 {
        boot_secondary_cpus(num_cpus);
    }

    // 7. 初始化定时器（在从核心启动后）
    timer::init();

    // 8. 启动第一个任务
    rest_init();
}
```

### 2.2 从核心启动流程

从核心的启动由主核心通过 SBI HSM 接口触发：

```
boot_secondary_cpus() [主核心]
    ↓
对每个从核心调用 sbi_hart_start()
    ↓
entry.S (secondary_sbi_entry) [从核心]
    ↓
设置页表、栈
    ↓
secondary_start() [从核心]
    ↓
初始化 trap 处理
    ↓
设置 tp 指向当前 CPU
    ↓
标记 CPU 在线
    ↓
进入 WFI 循环
```

#### 2.2.1 主核心侧：启动从核心

`boot_secondary_cpus()` 函数（第 388-439 行）负责启动所有从核心：

```rust
pub fn boot_secondary_cpus(num_cpus: usize) {
    if num_cpus <= 1 {
        pr_info!("[SMP] Single CPU mode, skipping secondary boot");
        return;
    }

    pr_info!("[SMP] Booting {} secondary CPUs...", num_cpus - 1);

    // 主核标记上线
    CPU_ONLINE_MASK.fetch_or(1, Ordering::Release);

    // 使用 SBI HSM 调用启动每个从核
    for hartid in 1..num_cpus {
        let start_vaddr = secondary_sbi_entry as usize;
        let start_paddr = unsafe { crate::arch::mm::vaddr_to_paddr(start_vaddr) };

        let ret = crate::arch::lib::sbi::hart_start(hartid, start_paddr, hartid);
        if ret.error != 0 {
            pr_err!("[SMP] Failed to start hart {}: SBI error {}", hartid, ret.error);
        }
    }

    // 等待所有核心上线（带超时）
    let expected_mask = (1 << num_cpus) - 1;
    let mut timeout = 10_000_000;

    while CPU_ONLINE_MASK.load(Ordering::Acquire) != expected_mask {
        if timeout == 0 {
            let current_mask = CPU_ONLINE_MASK.load(Ordering::Acquire);
            panic!(
                "[SMP] Timeout waiting for secondary CPUs! Expected: {:#b}, got: {:#b}",
                expected_mask, current_mask
            );
        }
        timeout -= 1;
        core::hint::spin_loop();
    }

    pr_info!("[SMP] All {} CPUs are online!", num_cpus);
}
```

#### 2.2.2 从核心侧：初始化和空挂

从核心从 `entry.S` 的 `secondary_sbi_entry` 开始执行，经过页表和栈设置后，调用 `secondary_start()` 函数（第 333-374 行）：

```rust
pub extern "C" fn secondary_start(hartid: usize) -> ! {
    // 1. 初始化 boot trap 处理
    trap::init_boot_trap();

    // 2. 设置 tp 指向当前 CPU 的 Cpu 结构
    {
        use crate::kernel::CPUS;
        let cpu_ptr = &*CPUS.get_of(hartid) as *const _ as usize;
        unsafe {
            core::arch::asm!("mv tp, {}", in(reg) cpu_ptr);
        }
    }

    // 3. 标记 CPU 在线
    CPU_ONLINE_MASK.fetch_or(1 << hartid, Ordering::Release);

    pr_info!("[SMP] CPU {} is online", hartid);

    // 4. 禁用中断
    unsafe {
        crate::arch::intr::disable_interrupts();
    }

    pr_info!("[SMP] CPU {} entering WFI loop with interrupts disabled", hartid);

    // 5. 进入 WFI 循环
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
```

### 2.3 启动同步机制

主核心和从核心之间通过原子变量 `CPU_ONLINE_MASK` 进行同步：

```rust
/// CPU 在线掩码，第 i 位为 1 表示 CPU i 已上线
static CPU_ONLINE_MASK: AtomicUsize = AtomicUsize::new(0);
```

- **从核心**：启动完成后，通过 `fetch_or` 设置自己的位
- **主核心**：轮询 `CPU_ONLINE_MASK`，等待所有核心的位都被设置（带超时）

## 3. Per-CPU 数据结构

### 3.1 Cpu 结构体

每个 CPU 核心都有一个独立的 `Cpu` 结构体，定义在 [`os/src/kernel/cpu.rs`](/os/src/kernel/cpu.rs)：

```rust
#[repr(C)]
pub struct Cpu {
    pub cpu_id: usize,                    // 必须是第一个字段，用于快速访问
    pub current_task: Option<SharedTask>,
    pub current_memory_space: Option<Arc<SpinLock<MemorySpace>>>,
}
```

**设计要点**：
- `cpu_id` 必须是第一个字段，这样可以通过 `ld {}, 0(tp)` 快速读取 CPU ID
- 每个核心维护自己的当前任务和内存空间

### 3.2 CPUS 全局数组

所有 CPU 的 `Cpu` 结构体存储在全局数组中：

```rust
static CPUS: PerCpu<Cpu> = PerCpu::new();
```

`PerCpu<T>` 是一个特殊的容器（定义在 [`os/src/sync/per_cpu.rs`](/os/src/sync/per_cpu.rs)），它：
- 为每个 CPU 分配独立的数据副本
- 使用缓存行对齐（64 字节），避免伪共享（False Sharing）
- 提供安全的访问接口

### 3.3 tp 寄存器的使用

RISC-V 的 tp (Thread Pointer) 寄存器在 Comix 内核中有双重用途：

| 模式       | tp 指向的内容                | 用途                                      |
|------------|------------------------------|-------------------------------------------|
| 内核模式   | 当前 CPU 的 `Cpu` 结构体指针 | 快速访问 Per-CPU 数据                     |
| 用户模式   | 用户线程的 TLS 指针          | 预留给未来的用户态线程局部存储（当前仅保存/恢复） |

**注意**：用户模式下的 TLS 功能当前仅在寄存器层面进行保存和恢复，完整的 TLS 支持尚未实现。详见第 4 节。

#### 3.3.1 快速访问 CPU ID

由于 `cpu_id` 是 `Cpu` 结构体的第一个字段（偏移 0），可以通过一条指令读取：

```rust
pub fn cpu_id() -> usize {
    let id: usize;
    unsafe {
        core::arch::asm!("ld {}, 0(tp)", out(reg) id);
    }
    id
}
```

#### 3.3.2 访问当前 CPU

```rust
pub fn current_cpu() -> &'static Cpu {
    let ptr: usize;
    unsafe {
        core::arch::asm!("mv {}, tp", out(reg) ptr);
        &*(ptr as *const Cpu)
    }
}
```

## 4. tp 寄存器的保存与恢复

### 4.1 问题背景

在多核系统中，tp 寄存器需要同时满足两个需求：
1. **内核需求**：快速访问当前 CPU 的 Per-CPU 数据
2. **用户需求**：为未来的用户态线程局部存储（TLS）预留支持

**重要说明**：当前实现**仅在寄存器层面**保存和恢复了 tp 的值，**并未实现完整的 TLS 功能**。完整的 TLS 实现还需要：
- 加载和解析 ELF 文件中的 TLS 段（`.tdata`, `.tbss`）
- 为每个线程分配 TLS 内存区域
- 初始化 TLS 变量
- 在线程创建时设置正确的 tp 值

当前的工作只是在 trap 入口和出口时切换 tp 的值，为未来的 TLS 支持奠定了寄存器层面的基础。

### 4.2 TrapFrame 中的 tp 字段

`TrapFrame` 结构体（定义在 [`os/src/arch/riscv/trap/trap_frame.rs`](/os/src/arch/riscv/trap/trap_frame.rs)）包含两个与 tp 相关的字段：

```rust
#[repr(C)]
pub struct TrapFrame {
    // ... 其他寄存器 ...
    pub x4_tp: usize,      // 用户态 tp（偏移 32）
    // ... 其他字段 ...
    pub cpu_ptr: usize,    // 内核态 tp，指向 Cpu 结构（偏移 272）
}
```

### 4.3 trap_entry.S 中的 tp 切换

在 trap 入口（[`os/src/arch/riscv/trap/trap_entry.S`](/os/src/arch/riscv/trap/trap_entry.S)，第 15-19 行），保存用户 tp 并加载内核 tp：

```asm
trap_entry:
    # a0 已经指向 TrapFrame（由 sscratch 提供）

    # 保存用户 tp 到 TrapFrame.x4_tp（偏移 32）
    sd tp, 32(a0)

    # 加载内核 tp 从 TrapFrame.cpu_ptr（偏移 272）
    ld tp, 272(a0)

    # ... 保存其他寄存器 ...
```

在 trap 返回时（`trap_return`），恢复用户 tp：

```asm
trap_return:
    # ... 恢复其他寄存器 ...

    # 恢复用户 tp 从 TrapFrame.x4_tp
    ld tp, 32(a0)

    # 返回用户态
    sret
```

### 4.4 TrapFrame 初始化

为了确保 `cpu_ptr` 字段正确初始化，`TrapFrame::zero_init()` 会自动设置它：

```rust
impl TrapFrame {
    pub fn zero_init() -> Self {
        let cpu_ptr = {
            let _guard = PreemptGuard::new();
            crate::kernel::current_cpu() as *const _ as usize
        };

        TrapFrame {
            // ... 所有字段初始化为 0 ...
            cpu_ptr,  // 设置为当前 CPU
        }
    }
}
```

对于内核线程，`set_kernel_trap_frame()` 还会将 `x4_tp` 设置为内核 tp：

```rust
pub fn set_kernel_trap_frame(&mut self, entry: usize, arg: usize, kstack_base: usize) {
    // ... 其他设置 ...

    self.x4_tp = {
        let _guard = PreemptGuard::new();
        crate::kernel::current_cpu() as *const _ as usize
    };
}
```

## 5. 从核心空挂

### 5.1 为什么空挂

当前从核心启动后立即进入 WFI（Wait For Interrupt）循环，原因是：

1. **调度器未就绪**：当前的调度器是单核设计，不支持多核任务分配
2. **避免竞争**：如果从核心尝试运行任务，会与主核心产生数据竞争
3. **节能**：WFI 指令让 CPU 进入低功耗状态，直到中断到来

### 5.2 为什么禁用中断

从核心在 WFI 循环前禁用中断（`disable_interrupts()`），原因是：

1. **避免意外唤醒**：禁用中断后，WFI 不会被时钟中断等唤醒
2. **简化状态**：从核心处于完全静止状态，不会执行任何代码
3. **等待显式唤醒**：未来的多核调度器可以通过 IPI（处理器间中断）显式唤醒从核心

### 5.3 未来的多核调度器

要让从核心参与任务调度，需要实现：

1. **Per-CPU 运行队列**：每个核心维护自己的就绪任务队列
2. **负载均衡**：在核心之间分配任务，避免某些核心空闲
3. **IPI 支持**：通过 IPI 唤醒空闲核心或请求任务迁移
4. **TLB Shootdown**：修改页表后，通过 IPI 通知其他核心刷新 TLB
5. **锁优化**：减少锁竞争，使用 Per-CPU 数据结构

## 6. 关键代码路径

### 6.1 文件列表

| 文件路径 | 功能描述 |
|----------|----------|
| [`os/src/arch/riscv/boot/mod.rs`](/os/src/arch/riscv/boot/mod.rs) | 主核心和从核心的启动逻辑 |
| [`os/src/arch/riscv/boot/entry.S`](/os/src/arch/riscv/boot/entry.S) | 汇编入口点（`_start`, `secondary_sbi_entry`） |
| [`os/src/kernel/cpu.rs`](/os/src/kernel/cpu.rs) | `Cpu` 结构体和访问函数 |
| [`os/src/sync/per_cpu.rs`](/os/src/sync/per_cpu.rs) | `PerCpu<T>` 容器实现 |
| [`os/src/arch/riscv/trap/trap_frame.rs`](/os/src/arch/riscv/trap/trap_frame.rs) | `TrapFrame` 结构体 |
| [`os/src/arch/riscv/trap/trap_entry.S`](/os/src/arch/riscv/trap/trap_entry.S) | trap 入口汇编（tp 切换） |
| [`os/src/arch/riscv/sbi.rs`](/os/src/arch/riscv/sbi.rs) | SBI HSM 接口封装 |
| [`os/src/config.rs`](/os/src/config.rs) | `MAX_CPU_COUNT` 配置 |

### 6.2 关键函数

| 函数名 | 位置 | 功能 |
|--------|------|------|
| `main()` | `boot/mod.rs:223` | 主核心启动入口 |
| `boot_secondary_cpus()` | `boot/mod.rs:397` | 启动所有从核心 |
| `secondary_start()` | `boot/mod.rs:350` | 从核心启动入口 |
| `init_cpus()` | `kernel/cpu.rs` | 初始化 CPUS 数组 |
| `cpu_id()` | `kernel/cpu.rs` | 获取当前 CPU ID |
| `current_cpu()` | `kernel/cpu.rs` | 获取当前 CPU 结构 |
| `TrapFrame::zero_init()` | `trap/trap_frame.rs` | 初始化 TrapFrame |

### 6.3 代码示例：启动 4 核系统

假设在 QEMU 中启动 4 核系统（`-smp 4`），启动过程如下：

```
时刻 T0: CPU 0 从 _start 开始执行
时刻 T1: CPU 0 初始化内存、CPUS、页表
时刻 T2: CPU 0 调用 boot_secondary_cpus()
时刻 T3: CPU 0 通过 SBI 启动 CPU 1, 2, 3
时刻 T4: CPU 1 从 secondary_sbi_entry 开始执行
时刻 T5: CPU 2 从 secondary_sbi_entry 开始执行
时刻 T6: CPU 3 从 secondary_sbi_entry 开始执行
时刻 T7: CPU 1 设置 tp，标记在线，进入 WFI
时刻 T8: CPU 2 设置 tp，标记在线，进入 WFI
时刻 T9: CPU 3 设置 tp，标记在线，进入 WFI
时刻 T10: CPU 0 检测到 CPU_ONLINE_MASK == 0b1111，继续初始化
时刻 T11: CPU 0 启动第一个任务，开始调度
```

## 7. 测试

多核启动的测试位于：
- [`os/src/arch/riscv/boot/mod.rs`](/os/src/arch/riscv/boot/mod.rs)（第 296-331 行）
- [`os/src/kernel/cpu.rs`](/os/src/kernel/cpu.rs)（第 108-196 行）

主要测试项：
- `test_num_cpu`：验证 `NUM_CPU` 正确设置
- `test_cpu_online_mask`：验证所有 CPU 都上线
- `test_cpus_initialization`：验证 CPUS 数组初始化
- `test_cpu_id`：测试 CPU ID 读取
- `test_per_cpu_independence`：验证 Per-CPU 数据独立性

运行测试：
```bash
cd os && make test
```

## 8. 限制和未来工作

### 8.1 当前限制

- 最多支持 8 个核心
- 从核心不参与任务调度
- 没有负载均衡
- 没有 IPI 支持
- 没有 TLB Shootdown

### 8.2 未来工作

1. **多核调度器**
   - Per-CPU 运行队列
   - 任务迁移和负载均衡
   - 优先级调度

2. **IPI 支持**
   - 唤醒空闲核心
   - TLB Shootdown
   - 系统停机协调

3. **性能优化**
   - 减少锁竞争
   - Per-CPU 缓存
   - NUMA 感知（如果硬件支持）

4. **调试支持**
   - Per-CPU 日志缓冲区
   - 多核死锁检测
   - 性能计数器

## 9. 参考资料

- [SMP 与中断](../sync/smp_interrupts.md) - SMP 系统中的中断和并发问题
- [Per-CPU 变量](../sync/per_cpu.md) - Per-CPU 数据结构的详细说明
- [抢占控制](../sync/preempt.md) - 访问 Per-CPU 数据时的抢占保护
- [RISC-V 寄存器](./riscv_register.md) - tp 寄存器的详细说明
- [RISC-V SBI Specification](https://github.com/riscv-non-isa/riscv-sbi-doc) - SBI HSM 接口规范
