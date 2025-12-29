# ComixOS 多核启动支持实现记录

## 任务目标

实现 GitHub Issue #209：多核启动支持（Multi-core Boot Support）

支持 2-8 核启动，主核完成系统初始化，从核等待主核完成后启动，每个核心有独立的启动栈和 CPU ID。

## 已完成的工作

### 1. 修改 CPU ID 获取机制

**文件**: `os/src/arch/riscv/kernel/cpu.rs`

将 `cpu_id()` 函数从硬编码返回 0 改为从 `tp` 寄存器读取：

```rust
#[inline]
pub fn cpu_id() -> usize {
    let id: usize;
    unsafe {
        core::arch::asm!("mv {}, tp", out(reg) id);
    }

    // 边界检查：如果 tp 值无效（被用户程序修改），返回 0
    let num_cpu = unsafe { crate::kernel::NUM_CPU };
    if id >= num_cpu {
        0  // 默认返回主核 ID
    } else {
        id
    }
}
```

**关键点**:
- 使用 RISC-V 的 `tp` (x4) 寄存器存储 CPU ID
- 添加边界检查，防止用户程序修改 tp 导致越界
- 如果 tp 值无效，默认返回 0（主核 ID）

### 2. 修改启动汇编

**文件**: `os/src/arch/riscv/boot/entry.S`

添加主从核分支逻辑：

#### 主核路径
```asm
_start:
    la t0, DTP
    sd a1, 0(t0)

    # 检查 hartid，主核继续，从核等待
    bnez a0, .secondary_hart_wait

.primary_hart:
    # ... 设置页表、跳转到高地址 ...

_start_high:
    la sp, boot_stack_top
    li t0, -1
    slli t0, t0, 38
    or sp, sp, t0

    # 设置主核 CPU ID
    li tp, 0

    call rust_main
```

#### 从核等待和启动路径
```asm
.secondary_hart_wait:
    mv s0, a0               # 保存 hartid

.wait_loop:
    wfi                     # 低功耗等待
    la t0, secondary_boot_flag
    ld t1, 0(t0)
    beqz t1, .wait_loop     # 标志为 0 则继续等待

    # 设置页表
    la t0, boot_pagetable
    srli t0, t0, 12
    li t1, 8 << 60
    or t0, t0, t1
    csrw satp, t0
    sfence.vma

    # 跳转到高地址
    la t0, .secondary_start_high
    li t1, -1
    slli t1, t1, 38
    or t0, t0, t1
    jr t0

.secondary_start_high:
    # 计算从核栈：secondary_stacks_top - hartid × 64KB
    la sp, secondary_stacks_top
    li t1, -1
    slli t1, t1, 38
    or sp, sp, t1

    # 使用移位代替乘法：hartid × 64KB = hartid << 16
    slli t1, s0, 16
    sub sp, sp, t1

    # 设置从核 CPU ID
    mv tp, s0

    # 调用从核入口
    mv a0, s0
    call secondary_start
```

#### 从核栈空间
```asm
.section .bss.secondary_stack
.align 12
secondary_stacks_bottom:
    .space 65536 * 8        # 8 核 × 64KB
.globl secondary_stacks_top
secondary_stacks_top:
```

**关键点**:
- 主核（hartid == 0）继续正常启动流程
- 从核在 WFI 中等待 `secondary_boot_flag` 被设置
- 每个从核有独立的 64KB 栈空间
- 使用移位操作 `slli t1, s0, 16` 代替乘法（因为 64KB = 2^16）
- 从核设置自己的 tp 寄存器为 hartid

### 3. 实现从核启动函数

**文件**: `os/src/arch/riscv/boot/mod.rs`

#### 添加静态变量
```rust
use core::sync::atomic::{AtomicUsize, Ordering};

/// 已上线 CPU 位掩码
static CPU_ONLINE_MASK: AtomicUsize = AtomicUsize::new(0);

/// 从核启动标志（对应 entry.S 中的 secondary_boot_flag）
#[unsafe(no_mangle)]
static mut secondary_boot_flag: u64 = 0;
```

#### 从核入口函数
```rust
#[unsafe(no_mangle)]
pub extern "C" fn secondary_start(hartid: usize) -> ! {
    // 验证 CPU ID
    let cpu = crate::arch::kernel::cpu::cpu_id();
    if cpu != hartid {
        panic!("CPU ID mismatch! tp={}, hartid={}", cpu, hartid);
    }

    pr_info!("[SMP] Secondary hart {} starting...", hartid);

    // 初始化 trap 处理
    trap::init();

    // 标记当前 CPU 上线
    CPU_ONLINE_MASK.fetch_or(1 << hartid, Ordering::Release);

    pr_info!("[SMP] CPU {} is online", hartid);

    // 进入空闲循环（等待调度器实现）
    // 注意：不启用定时器和中断，因为还没有任务上下文
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
```

**关键点**:
- 验证 tp 寄存器中的 CPU ID 与 hartid 一致
- 只初始化 trap 处理，不启用定时器和中断（避免在没有任务上下文时触发中断）
- 使用原子操作标记 CPU 上线
- 进入 WFI 循环等待调度器实现

#### 主核启动从核函数
```rust
pub fn boot_secondary_cpus(num_cpus: usize) {
    if num_cpus <= 1 {
        pr_info!("[SMP] Single CPU mode, skipping secondary boot");
        return;
    }

    pr_info!("[SMP] Booting {} secondary CPUs...", num_cpus - 1);

    // 主核标记上线
    CPU_ONLINE_MASK.fetch_or(1, Ordering::Release);

    // 设置从核启动标志
    unsafe {
        let ptr = core::ptr::addr_of_mut!(secondary_boot_flag);
        core::ptr::write_volatile(ptr, 1);
        core::sync::atomic::fence(Ordering::Release);
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

**关键点**:
- 使用 `write_volatile` 和内存屏障确保标志位对所有核心可见
- 使用位掩码跟踪 CPU 上线状态
- 带超时机制，防止无限等待
- 使用 Acquire/Release 内存顺序保证同步

#### 修改主核启动流程
```rust
pub fn main(hartid: usize) {
    clear_bss();
    run_early_tests();

    earlyprintln!("[Boot] Hello, world!");
    earlyprintln!("[Boot] RISC-V Hart {} is up!", hartid);

    mm::init();

    #[cfg(test)]
    crate::test_main();

    trap::init_boot_trap();
    platform::init();  // 这里会设置 NUM_CPU
    time::init();
    timer::init();
    unsafe { intr::enable_interrupts() };

    // 启动从核
    let num_cpus = unsafe { NUM_CPU };
    if num_cpus > 1 {
        boot_secondary_cpus(num_cpus);
    }

    rest_init();
}
```

### 4. 修改 QEMU 运行脚本

**文件**: `os/qemu-run.sh`

修改 SMP 参数从环境变量读取：

```bash
# 参数定义
os_file="$BIN_FILE"
mem="4G"
smp="${SMP:-1}"  # 从环境变量读取，默认为 1
fs="fs.img"
disk="disk.img"
```

**使用方法**:
```bash
SMP=2 make run  # 2 核启动
SMP=4 make run  # 4 核启动
```

### 5. 修复 trap 处理中的 tp 寄存器恢复

**文件**: `os/src/arch/riscv/trap/trap_entry.S`

注释掉 tp 寄存器的恢复，防止用户程序修改的 tp 值被恢复：

```asm
__restore:
    # ... 恢复其他寄存器 ...

    # 不恢复 tp (x4)，保持内核设置的 CPU ID
    # ld tp, 32(a0)

    # ... 恢复其他寄存器 ...
    sret
```

## 当前问题

### 问题：CPUS 向量初始化时机问题

**现象**: 运行 2 核测试时，系统在 `[SMP] Booting 1 secondary CPUs...` 后立即 panic：
```
Panicked at src/kernel/task/mod.rs:83 current_task: CPU has no current task
```

**根本原因**:
1. `CPUS` 是通过 `lazy_static!` 初始化的 `Vec<SpinLock<Cpu>>`
2. 初始化时读取 `NUM_CPU` 的值来决定创建多少个 CPU 实例
3. 但 `CPUS` 第一次被访问时（在 `mm::init()` 中），`NUM_CPU` 还是默认值 1
4. 因此 `CPUS` 只创建了 1 个 CPU 实例（CPU 0）
5. 当从核（CPU 1）启动并尝试访问 `CPUS[1]` 时，发生越界

**启动顺序**:
```
main()
  ├─ mm::init()              # 第一次访问 CPUS，此时 NUM_CPU = 1
  │   └─ current_cpu()       # 触发 CPUS 初始化，只创建 CPUS[0]
  ├─ platform::init()        # 设置 NUM_CPU = 2（太晚了！）
  └─ boot_secondary_cpus()   # 从核尝试访问 CPUS[1]，越界！
```

## 解决方案（已确定）

### 方案：使用固定大小数组代替 Vec

**用户建议**：直接初始化 MAX_CPU_COUNT 个 CPU 实例，做成数组而非 vector，避免 SpinLock 对多核性能的影响。

#### 为什么使用固定大小数组？

**性能优势**：
- 每个 CPU 只访问自己的 SpinLock，无全局锁竞争
- 编译时确定大小，无运行时分配开销
- 缓存友好：数组连续存储，局部性更好
- 无需 lazy_static 的初始化开销

**多核扩展性**：
- Vec + SpinLock 方案：所有 CPU 竞争同一个锁，扩展性差 O(n)
- 数组方案：每个 CPU 独立锁，O(1) 扩展性

**实现简洁**：
- 使用 `const fn new_const()` 支持编译时初始化
- 无需复杂的生命周期管理
- 代码更简单，更易维护

**内存开销**：
- 数组大小：MAX_CPU_COUNT × sizeof(SpinLock<Cpu>)
- 假设 MAX_CPU_COUNT = 8，每个 Cpu 约 32 字节
- 总开销：约 256 字节（完全可接受）

#### 修改 CPUS 结构

**文件**: `os/src/kernel/cpu.rs`

```rust
use crate::{
    config::MAX_CPU_COUNT,
    kernel::task::SharedTask,
    mm::memory_space::MemorySpace,
    sync::SpinLock,
};

pub static mut NUM_CPU: usize = 1;
pub static mut CLOCK_FREQ: usize = 12_500_000;

/// CPU 数组，固定大小为 MAX_CPU_COUNT
/// 每个 CPU 只访问自己的位置，无需全局锁
static CPUS: [SpinLock<Cpu>; MAX_CPU_COUNT] = {
    const INIT: SpinLock<Cpu> = SpinLock::new(Cpu::new_const());
    [INIT; MAX_CPU_COUNT]
};

/// CPU 结构体
pub struct Cpu {
    pub current_task: Option<SharedTask>,
    pub current_memory_space: Option<Arc<SpinLock<MemorySpace>>>,
}

impl Cpu {
    /// 创建一个新的 CPU 实例（const 版本，用于静态初始化）
    pub const fn new_const() -> Self {
        Cpu {
            current_task: None,
            current_memory_space: None,
        }
    }

    /// 创建一个新的 CPU 实例
    pub fn new() -> Self {
        Cpu {
            current_task: None,
            current_memory_space: None,
        }
    }

    // ... 其他方法保持不变 ...
}

/// 获取当前 CPU 的引用
pub fn current_cpu() -> &'static SpinLock<Cpu> {
    let cpu_id = crate::arch::kernel::cpu::cpu_id();
    &CPUS[cpu_id]
}
```

**关键点**：
- 使用 `const fn new_const()` 支持编译时初始化
- 数组大小为 `MAX_CPU_COUNT`，支持最多 8 核
- 每个 CPU 独立的 SpinLock，避免锁竞争
- 无需 `lazy_static!`，编译时确定
- 无需从核动态添加，所有 CPU 实例已预先创建

**优点**：
- 性能最优：无全局锁竞争
- 实现最简单：无需动态管理
- 内存开销可控：固定 256 字节
- 符合 Per-CPU 设计理念

## 测试计划

1. **编译测试**: 确保代码编译通过
2. **单核测试**: `make run` 验证单核模式正常工作
3. **2 核测试**: `SMP=2 make run` 验证双核启动
4. **4 核测试**: `SMP=4 make run` 验证四核启动
5. **GDB 调试**: 使用 GDB 验证 tp 寄存器和栈指针设置正确

## 后续工作

完成多核启动后，下一步工作：
1. Per-CPU 调度器（Issue #210）
2. IPI 核间中断支持（Issue #211）
3. TLB shootdown 机制
4. 负载均衡

## 参考资料

- RISC-V 特权级规范：关于 hartid 和 CPU 启动
- RISC-V ABI 规范：关于 tp 寄存器的使用
- OpenSBI 文档：关于从核启动流程
- Linux RISC-V 实现：参考多核启动机制

## 2024-12-28: TLS 支持和 CPUS 重构

### 背景

根据 SMP_IMPLEMENTATION_PLAN.md，需要重构 CPUS 以支持用户态 TLS（Thread Local Storage）。主要问题：
1. CPUS 初始化时机错误（NUM_CPU=1 时初始化）
2. 缓存行伪共享（Vec<SpinLock<Cpu>> 无对齐）
3. Per-CPU 数据不应该需要锁

### 步骤 0: 重构 trap 处理以支持 TLS

#### 0.1 在 TrapFrame 中添加 cpu_ptr 字段

**文件**: `os/src/arch/riscv/trap/trap_frame.rs`

在 TrapFrame 末尾添加 cpu_ptr 字段（偏移 272）：

```rust
#[repr(C)]
pub struct TrapFrame {
    pub sepc: usize,        // 0
    pub ra: usize,          // 8
    // ... 其他寄存器 ...
    pub sstatus: usize,     // 256
    pub kernel_sp: usize,   // 264
    pub cpu_ptr: usize,     // 272 - 指向当前 CPU 结构体
}
```

#### 0.2 修改 trap_entry.S

**文件**: `os/src/arch/riscv/trap/trap_entry.S`

修改 trap entry 保存用户 tp 并加载内核 tp：

```asm
trap_entry:
    csrrw a0, sscratch, a0
    sd tp, 32(a0)           # 保存用户 tp
    ld tp, 272(a0)          # 加载 cpu_ptr 到 tp
    # ... 保存其他寄存器 ...
```

修改 __restore 恢复用户 tp：

```asm
__restore:
    # ... 恢复其他寄存器 ...
    ld tp, 32(a0)           # 恢复用户 tp
    ld a0, 80(a0)
    sret
```

#### 0.3 修改 cpu_id() 函数

**文件**: `os/src/arch/riscv/kernel/cpu.rs`

从 tp 指向的 Cpu 结构体读取 cpu_id：

```rust
pub fn cpu_id() -> usize {
    let id: usize;
    unsafe {
        core::arch::asm!("ld {}, 0(tp)", out(reg) id);
    }
    id
}
```

#### 0.4 更新 Cpu 结构体

**文件**: `os/src/kernel/cpu.rs`

确保 cpu_id 是第一个字段：

```rust
#[repr(C)]
pub struct Cpu {
    pub cpu_id: usize,  // 必须是第一个字段
    pub current_task: Option<SharedTask>,
    pub current_memory_space: Option<Arc<SpinLock<MemorySpace>>>,
}

impl Cpu {
    pub fn new_with_id(cpu_id: usize) -> Self {
        Cpu {
            cpu_id,
            current_task: None,
            current_memory_space: None,
        }
    }
}
```

将 CPUS 改为 PerCpu<Cpu>：

```rust
lazy_static! {
    pub static ref CPUS: PerCpu<Cpu> = {
        PerCpu::new_with_id(|cpu_id| Cpu::new_with_id(cpu_id))
    };
}

pub fn current_cpu() -> &'static mut Cpu {
    CPUS.get_mut()
}
```

#### 0.5 在任务创建时设置 TrapFrame.cpu_ptr

**文件**: `os/src/kernel/task/ktask.rs`, `os/src/kernel/task/task_struct.rs`

在 kthread_spawn() 和 execve() 中设置 cpu_ptr：

```rust
unsafe {
    (*tf).set_kernel_trap_frame(...);
    let cpu_ptr = {
        let _guard = crate::sync::PreemptGuard::new();
        crate::kernel::current_cpu() as *const _ as usize
    };
    (*tf).cpu_ptr = cpu_ptr;
}
```

### 遇到的问题和解决方案

#### 问题 1: current_memory_space panic

**现象**: 设备初始化时 panic "current_memory_space: current task has no memory space"

**原因**: mm::init() 中移除了 `current_cpu().switch_space(space)` 调用，导致 current_memory_space 没有设置

**解决方案**:
1. 修改 mm::init() 返回创建的 space
2. 在 boot/mod.rs 中初始化 CPUS 后调用 switch_space

```rust
// boot/mod.rs
pub fn main(hartid: usize) {
    let kernel_space = mm::init();

    // 初始化 CPUS 并设置 tp
    {
        use crate::kernel::CPUS;
        let cpu_ptr = &*CPUS.get_of(0) as *const _ as usize;
        unsafe {
            core::arch::asm!("mv tp, {}", in(reg) cpu_ptr);
        }
    }

    // 激活内核地址空间并设置 current_memory_space
    {
        let _guard = crate::sync::PreemptGuard::new();
        current_cpu().switch_space(kernel_space);
    }

    // ... 其余初始化 ...
}
```

#### 问题 2: 从核启动超时

**现象**: 从核没有启动，超时 panic "Expected: 0b1111, got: 0b1"

**根本原因分析**:

1. **tp 寄存器问题**:
   - entry.S 中从核设置 `tp = hartid`（小整数 1, 2, 3）
   - PreemptGuard::new() → preempt_disable() → cpu_id()
   - cpu_id() 从 tp 指向的地址读取，但 tp 不是有效指针
   - 导致从核在调用任何日志函数时就崩溃

   **解决**: 移除 entry.S 中的 `mv tp, s0`，让 secondary_start() 第一时间设置正确的 tp

2. **secondary_boot_flag 地址问题**:
   - 原本定义在 Rust 代码中（虚拟地址空间）
   - 从核在物理地址模式下无法访问

   **解决**: 将 secondary_boot_flag 移到 entry.S 的 .data 段定义

3. **从核分页启用问题**:
   - 从核在物理地址模式下使用 `la` 加载虚拟地址
   - 需要先启用分页再检查 secondary_boot_flag

   **解决**: 从核先启用分页，跳转到高地址，再检查标志

### 当前修改

#### entry.S 修改

```asm
.secondary_hart_wait:
    mv s0, a0               # 保存 hartid

    # 1. 先启用分页
    la t0, boot_pagetable
    li t1, 0x3FFFFFFFFF
    and t0, t0, t1          # 转换为物理地址
    srli t0, t0, 12
    li t1, 8 << 60
    or t0, t0, t1
    csrw satp, t0
    sfence.vma

    # 2. 跳转到高地址
    la t0, .secondary_wait_high
    li t1, -1
    slli t1, t1, 38
    or t0, t0, t1
    jr t0

.secondary_wait_high:
    # 3. 设置栈并调用调试函数
    la sp, secondary_stacks_top
    li t1, -1
    slli t1, t1, 38
    or sp, sp, t1
    slli t1, s0, 16
    sub sp, sp, t1

    mv a0, s0
    call secondary_debug_entry

.wait_loop:
    wfi
    la t0, secondary_boot_flag
    li t1, -1
    slli t1, t1, 38
    or t0, t0, t1
    ld t1, 0(t0)
    beqz t1, .wait_loop

.secondary_start_high:
    # 计算栈地址
    # ... (不再设置 tp，由 secondary_start 设置)
    mv a0, s0
    call secondary_start
```

#### boot/mod.rs 修改

```rust
// secondary_boot_flag 改为 extern 声明
unsafe extern "C" {
    static mut secondary_boot_flag: u64;
}

// 添加调试函数
#[unsafe(no_mangle)]
pub extern "C" fn secondary_debug_entry(hartid: usize) {
    crate::earlyprintln!("[DEBUG] Hart {} reached secondary_wait_high", hartid);
}

// secondary_start 第一时间设置 tp
pub extern "C" fn secondary_start(hartid: usize) -> ! {
    // 首先设置 tp 指向对应的 Cpu 结构体
    {
        use crate::kernel::CPUS;
        let cpu_ptr = &*CPUS.get_of(hartid) as *const _ as usize;
        unsafe {
            core::arch::asm!("mv tp, {}", in(reg) cpu_ptr);
        }
    }

    // 验证 CPU ID
    let cpu = crate::arch::kernel::cpu::cpu_id();
    if cpu != hartid {
        panic!("CPU ID mismatch! cpu_id()={}, hartid={}", cpu, hartid);
    }

    // ... 其余初始化 ...
}
```

### 当前状态

**已完成**:
- ✅ TrapFrame 添加 cpu_ptr 字段
- ✅ trap_entry.S 支持 TLS
- ✅ cpu_id() 从 Cpu 结构体读取
- ✅ Cpu 结构体重构（cpu_id 第一个字段）
- ✅ 任务创建时设置 cpu_ptr
- ✅ mm::init() 返回 space
- ✅ boot/mod.rs 正确设置 current_memory_space
- ✅ 移除 entry.S 中从核的 tp 设置
- ✅ secondary_boot_flag 移到 entry.S
- ✅ 从核先启用分页再检查标志

**当前问题**:
- ❌ 从核仍然没有启动，没有任何调试输出
- 可能原因：从核在 entry.S 早期阶段卡住

**下一步调试**:
1. 使用 GDB 连接 QEMU 查看所有核心状态
2. 检查从核的 PC 寄存器位置
3. 简化从核启动流程，逐步验证每个阶段

### 测试日志

```bash
# 测试命令
SMP=4 make run

# 预期输出
[SMP] Booting 3 secondary CPUs...
[DEBUG] Hart 1 reached secondary_wait_high
[DEBUG] Hart 2 reached secondary_wait_high
[DEBUG] Hart 3 reached secondary_wait_high
[SMP] Secondary hart 1 starting...
[SMP] CPU 1 is online
[SMP] Secondary hart 2 starting...
[SMP] CPU 2 is online
[SMP] Secondary hart 3 starting...
[SMP] CPU 3 is online
[SMP] All 4 CPUs are online!

# 实际输出
[SMP] Booting 3 secondary CPUs...
Panicked at src/arch/riscv/boot/mod.rs:406 [SMP] Timeout waiting for secondary CPUs! Expected: 0b1111, got: 0b1
```

没有看到任何 DEBUG 消息，说明从核根本没有执行到 secondary_debug_entry。

### 关键发现

用户指出日志系统的 collect_context() 中 PreemptGuard::new() 会调用 cpu_id()，而从核的 tp 在 entry.S 中被设置为 hartid（无效指针），导致从核在任何日志调用时就崩溃。

---

## 2024-12-28: 最终解决方案 - SBI HSM 启动

### 问题根源

经过深入调试，发现了三个关键问题：

1. **虚拟地址 vs 物理地址**: 传给 SBI HSM 的地址必须是物理地址，但代码传的是虚拟地址
2. **缺少汇编入口点**: `secondary_start` 是 Rust 函数（虚拟地址），从核从物理地址启动时 MMU 关闭，无法执行
3. **CPUS 初始化时机**: `CPUS` 的 lazy_static 初始化时 `NUM_CPU` 还是默认值 1，导致只创建 1 个 CPU 实例

### 最终修复

#### 1. 物理地址转换

**文件**: `os/src/arch/riscv/boot/mod.rs`

```rust
// 使用 SBI HSM 调用启动每个从核
for hartid in 1..num_cpus {
    let start_vaddr = secondary_sbi_entry as usize;
    let start_paddr = unsafe { crate::arch::mm::vaddr_to_paddr(start_vaddr) };

    let ret = crate::arch::sbi::hart_start(hartid, start_paddr, hartid);
    if ret.error != 0 {
        pr_err!("[SMP] Failed to start hart {}: SBI error {}", hartid, ret.error);
    }
}
```

#### 2. 创建汇编入口点

**文件**: `os/src/arch/riscv/boot/entry.S`

```asm
# SBI HSM 从核入口
.globl secondary_sbi_entry
secondary_sbi_entry:
    mv s0, a0               # 保存 hartid

    # 1. 启用分页
    la t0, boot_pagetable
    li t1, 0x3FFFFFFFFF
    and t0, t0, t1          # 转换为物理地址
    srli t0, t0, 12
    li t1, 8 << 60
    or t0, t0, t1
    csrw satp, t0
    sfence.vma

    # 2. 跳转到高地址
    la t0, .secondary_sbi_high
    li t1, -1
    slli t1, t1, 38
    or t0, t0, t1
    jr t0

.secondary_sbi_high:
    # 3. 设置栈
    la sp, secondary_stacks_top
    li t1, -1
    slli t1, t1, 38
    or sp, sp, t1
    slli t1, s0, 16          # hartid × 64KB
    sub sp, sp, t1

    # 4. 跳转到 Rust 入口
    mv a0, s0
    call secondary_start
```

#### 3. 修复 CPUS 初始化

**文件**: `os/src/kernel/cpu.rs`

```rust
lazy_static! {
    pub static ref CPUS: PerCpu<Cpu> = {
        use crate::config::MAX_CPU_COUNT;
        // 使用 MAX_CPU_COUNT 而不是 NUM_CPU，避免初始化时机问题
        PerCpu::new_with_id_and_count(MAX_CPU_COUNT, |cpu_id| Cpu::new_with_id(cpu_id))
    };
}
```

**文件**: `os/src/sync/per_cpu.rs`

```rust
pub fn new_with_id_and_count<F: Fn(usize) -> T>(count: usize, init: F) -> Self {
    assert!(count > 0, "CPU count must be greater than 0");

    let mut data = Vec::with_capacity(count);
    for i in 0..count {
        data.push(CacheAligned::new(init(i)));
    }
    PerCpu { data }
}
```

#### 4. 从核启动函数

**文件**: `os/src/arch/riscv/boot/mod.rs`

```rust
#[unsafe(no_mangle)]
pub extern "C" fn secondary_start(hartid: usize) -> ! {
    // 设置 tp 指向对应的 Cpu 结构体
    {
        use crate::kernel::CPUS;
        let cpu_ptr = &*CPUS.get_of(hartid) as *const _ as usize;
        unsafe {
            core::arch::asm!("mv tp, {}", in(reg) cpu_ptr);
        }
    }

    // 初始化 trap 处理
    trap::init();

    // 标记当前 CPU 上线
    CPU_ONLINE_MASK.fetch_or(1 << hartid, Ordering::Release);

    pr_info!("[SMP] CPU {} is online", hartid);

    // 进入 WFI 循环，等待多核调度器实现
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
```

### 测试结果

```bash
SMP=4 make run

# 输出
[SMP] Booting 3 secondary CPUs...
[SMP] Starting hart 1 at vaddr=0xffffffc0802000fc, paddr=0x802000fc
[SMP] Hart 1 SBI call succeeded
[SMP] Starting hart 2 at vaddr=0xffffffc0802000fc, paddr=0x802000fc
[SMP] Hart 2 SBI call succeeded
[SMP] Starting hart 3 at vaddr=0xffffffc0802000fc, paddr=0x802000fc
[SMP] Hart 3 SBI call succeeded
[INFO] [CPU1/T  0] [SMP] CPU 1 is online
[INFO] [CPU2/T  0] [SMP] CPU 2 is online
[INFO] [CPU3/T  0] [SMP] CPU 3 is online
[INFO] [CPU0/T  0] [SMP] All 4 CPUs are online!
```

### 成功标志

✅ 所有 4 个 CPU 成功上线
✅ 每个 CPU 有独立的 tp 指向自己的 Cpu 结构体
✅ TLS 支持基础架构完成（TrapFrame.cpu_ptr, trap_entry/restore）
✅ 从核进入 WFI 循环，等待多核调度器实现

### 关键经验

1. **SBI HSM 地址必须是物理地址**: OpenSBI 将从核跳转到物理地址时 MMU 关闭
2. **需要汇编入口点**: 从核启动时需要先启用分页、跳转到高地址，然后才能执行 Rust 代码
3. **lazy_static 初始化时机**: 使用 `MAX_CPU_COUNT` 而不是运行时的 `NUM_CPU`
4. **Cpu 结构体布局**: 必须使用 `#[repr(C)]` 确保 `cpu_id` 在偏移 0 处

### 下一步

- 实现多核调度器，让从核能够运行任务
- 测试用户态 TLS 功能
- 实现核间中断（IPI）用于任务迁移和同步

---

## 2024-12-28: TLS 支持导致的 trap_entry 问题

### 问题发现

在实现 TLS 支持后，系统出现 "Kernel exception in S-Mode" 错误：
- Exception: 13 (Load Page Fault)
- Faulting VA (stval): 0x0
- sscratch: 0x1

### 根本原因

修改 trap_entry.S 添加 TLS 支持时，在第 19 行添加了：
```asm
ld tp, 272(a0)  # 从 TrapFrame 读取 cpu_ptr
```

问题：如果 sscratch 没有正确设置（例如还是初始值 0 或 1），`a0` 就是无效地址，导致 Load Page Fault。

### 时序问题

启动流程：
1. `main()` 调用 `trap::init_boot_trap()` - 设置 stvec 指向 boot_trap_entry
2. `main()` 调用 `platform::init()` - 可能触发中断
3. `main()` 调用 `enable_interrupts()` - 启用中断
4. `main()` 调用 `rest_init()` - 创建第一个任务
5. `rest_init()` 设置 sscratch 指向 TrapFrame
6. `init()` 调用 `trap::init()` - 切换到 trap_entry

**问题窗口**：在步骤 3-5 之间，如果发生中断，trap_entry 会尝试从无效的 sscratch 读取。

### 尝试的解决方案

1. **移除从核的 trap::init()**：从核不调用 trap::init()，继续使用 boot_trap_entry
   - 结果：单核和多核都还是有问题

2. **延迟 enable_interrupts()**：将中断启用移到 init() 函数中
   - 结果：还是有问题，说明异常发生在 rest_init() 之前

3. **回滚 trap_entry.S**：恢复到 commit 4b56028 的版本
   - 需要测试：是否能解决问题

### 当前状态

- ✅ 多核启动成功：所有 4 个 CPU 都能上线
- ✅ SBI HSM 机制工作正常
- ✅ 物理地址转换正确
- ✅ CPUS 初始化时机修复（使用 MAX_CPU_COUNT）
- ❌ TLS 支持导致 trap_entry 崩溃
- ❌ 单核模式也无法正常运行

### 问题分析

TLS 支持的设计存在根本性问题：
1. **假设 sscratch 总是有效**：trap_entry 假设 sscratch 总是指向有效的 TrapFrame
2. **启动早期没有 TrapFrame**：在 rest_init() 之前，没有任务，也没有 TrapFrame
3. **boot_trap_entry 和 trap_entry 混用**：启动早期使用 boot_trap_entry，后期切换到 trap_entry，但切换时机不明确

### 可能的解决方案

#### 方案 A：延迟 TLS 支持
- 暂时回滚 trap_entry.S 的 TLS 修改
- 先让多核启动工作
- 后续再实现 TLS 支持，需要更仔细的设计

#### 方案 B：修复 trap_entry 的健壮性
- 在 trap_entry 中添加 sscratch 有效性检查
- 如果 sscratch 无效，使用备用逻辑（类似 boot_trap_entry）
- 但这会增加 trap_entry 的复杂度和开销

#### 方案 C：确保 sscratch 总是有效
- 在 main() 开始时就分配一个临时 TrapFrame 并设置 sscratch
- 在 rest_init() 时替换为真正的 TrapFrame
- 需要额外的内存分配和管理

### 推荐方案

**采用方案 A**：暂时回滚 TLS 支持，先完成多核启动的基本功能。

理由：
1. TLS 支持不是多核启动的必需功能
2. 当前的 TLS 实现设计有缺陷，需要重新设计
3. 多核启动本身已经成功，不应该被 TLS 问题阻塞
4. 可以在后续 PR 中单独实现 TLS 支持

### 下一步行动

1. 回滚 trap_entry.S 到 commit 4b56028 的版本
2. 回滚 TrapFrame 的 cpu_ptr 字段
3. 测试单核和多核模式是否正常工作
4. 如果正常，提交多核启动的 PR
5. 在新的 PR 中重新设计和实现 TLS 支持

---

## 2024-12-28: 从核禁用中断修复

### 问题

从核在 WFI 循环中等待多核调度器实现，但没有完整的 trap 处理上下文：
- 从核没有调用 trap 初始化函数
- 从核没有 TrapFrame 和有效的 sscratch
- 如果从核收到中断，会尝试使用 boot_trap_entry 处理
- 可能因为 trap 处理不完整而崩溃（Load Page Fault）

### 解决方案

在从核进入 WFI 循环之前禁用中断：
- 调用 `disable_interrupts()` 清除 sstatus.SIE 位
- 从核不会响应任何中断，保持在 WFI 状态
- 等待多核调度器实现后再启用中断

### 修改内容

**文件**: `os/src/arch/riscv/boot/mod.rs`

在 `secondary_start()` 函数中添加：
```rust
// 禁用中断，避免在 WFI 循环中响应中断
// 等待多核调度器实现后再启用
unsafe {
    crate::arch::intr::disable_interrupts();
}

pr_info!("[SMP] CPU {} entering WFI loop with interrupts disabled", hartid);
```

更新文档注释：
```rust
/// # 注意事项
/// - 从核不初始化 trap 处理，继续使用 boot_trap_entry
/// - 从核禁用中断，避免在没有完整 trap 上下文时响应中断
/// - 等待多核调度器实现后，从核将被唤醒并启用中断
```

### 测试结果

```bash
SMP=4 make run

# 预期输出
[SMP] Booting 3 secondary CPUs...
[INFO] [CPU1/T  0] [SMP] CPU 1 is online
[INFO] [CPU1/T  0] [SMP] CPU 1 entering WFI loop with interrupts disabled
[INFO] [CPU2/T  0] [SMP] CPU 2 is online
[INFO] [CPU2/T  0] [SMP] CPU 2 entering WFI loop with interrupts disabled
[INFO] [CPU3/T  0] [SMP] CPU 3 is online
[INFO] [CPU3/T  0] [SMP] CPU 3 entering WFI loop with interrupts disabled
[INFO] [CPU0/T  0] [SMP] All 4 CPUs are online!
```

### 后续工作

- 实现多核调度器（Issue #210）
- 从核启用中断并开始调度任务
- 实现 IPI 核间中断支持（Issue #211）

---

## 2024-12-29: 修复 TLS 初始化问题

### 问题

TLS 支持的实现导致主核 Load Page Fault：
- trap_entry.S 从 TrapFrame.cpu_ptr 加载到 tp
- TrapFrame::zero_init() 将 cpu_ptr 默认初始化为 0
- 如果某处使用 zero_init() 创建 TrapFrame 但没有显式设置 cpu_ptr
- cpu_id() 尝试从地址 0 读取，导致 Load Page Fault

**错误分析**（通过 rust-objdump）：
```
ffffffc0802274d2: ld a3, 0(tp)  # cpu_id() 从 tp 读取 CPU ID
```

错误发生在 cpu_id() 函数，因为 tp 寄存器的值为 0（从未初始化的 cpu_ptr 加载）。

### 解决方案

修复 TrapFrame::zero_init() 函数，自动初始化 cpu_ptr：
- zero_init() 调用 current_cpu() 获取当前 CPU 的指针
- 所有通过 zero_init() 创建的 TrapFrame 都会自动有正确的 cpu_ptr
- 保留 TLS 支持的所有代码，无需回滚

### 修改内容

**文件**: `os/src/arch/riscv/trap/trap_frame.rs`

修改 zero_init() 函数：
```rust
pub fn zero_init() -> Self {
    // 获取当前 CPU 的指针
    let cpu_ptr = {
        use crate::sync::PreemptGuard;
        let _guard = PreemptGuard::new();
        crate::kernel::current_cpu() as *const _ as usize
    };

    TrapFrame {
        // ... 其他字段 ...
        cpu_ptr,  // 自动初始化
    }
}
```

### 优点

- 保留 TLS 支持，性能更好
- 自动初始化，不会遗漏
- 代码改动最小
- 向后兼容，不影响已有的显式初始化代码

### 测试结果

待测试...


---

## 2024-12-29: TLS 回滚实验及根本原因分析

### 背景

在尝试修复 TLS 初始化问题后，多核启动仍然失败。用户要求尝试回滚 TLS 实现，重新评估如何正确实现 TLS。

### 回滚方案

将 tp 寄存器从"指向 Cpu 结构体的指针"改为"直接存储 CPU ID 值"：

1. **移除 TrapFrame.cpu_ptr 字段**
   - 删除 `trap_frame.rs` 中的 cpu_ptr 字段
   - 简化 zero_init() 函数

2. **修改 trap_entry.S**
   - 删除从 TrapFrame 加载 cpu_ptr 到 tp 的代码
   - tp 寄存器不再在 trap entry 时被修改

3. **修改 cpu_id() 实现**
   ```rust
   #[inline]
   pub fn cpu_id() -> usize {
       let id: usize;
       unsafe {
           core::arch::asm!(
               "mv {}, tp",  // 直接读取 tp 寄存器的值
               out(reg) id
           );
       }
       id
   }
   ```

4. **修改启动代码**
   - 主核：`li tp, 0` (设置 tp 为 CPU ID 0)
   - 从核：`mv tp, {hartid}` (设置 tp 为对应的 hartid)

### 测试结果

回滚后的代码能够成功编译并启动，但在执行用户程序时崩溃：

```
Panicked at src/sync/preempt.rs:35 index out of bounds: the len is 8 but the index is 1187136
```

### 根本原因分析

**问题**：tp 寄存器无法同时服务于内核和用户态

1. **内核态需求**：tp 需要存储 CPU ID，用于 cpu_id() 函数
2. **用户态需求**：tp 是 RISC-V ABI 规定的 Thread Local Storage 指针

**崩溃原因**：
- 当内核执行 `__restore` 返回用户态时，恢复了用户的 tp 值（用户的 TLS 指针）
- 用户程序运行时，tp 包含用户的 TLS 地址（例如 0x121d00 = 1187136）
- 当用户程序触发系统调用或中断，trap 回内核时，tp 仍然是用户的值
- 内核代码调用 cpu_id() 时，直接读取 tp，得到错误的值 1187136
- 使用这个值作为 CPUS 数组的索引，导致 index out of bounds panic

**关键洞察**：
- RISC-V ABI 规定 tp 寄存器用于 TLS，用户程序会使用它
- 内核不能假设 tp 在 trap 时保持内核设置的值
- 简单的"tp 存储 CPU ID"方案与用户态 TLS 冲突

### 结论

回滚实验证明了原始 TLS 实现方向是正确的：

1. **必须在 trap entry 时设置 tp**
   - 不能依赖启动时设置的 tp 值
   - 必须在每次进入内核时重新设置 tp

2. **TrapFrame.cpu_ptr 字段是必要的**
   - 需要一个地方存储 CPU 指针
   - trap_entry.S 从 TrapFrame 加载 cpu_ptr 到 tp

3. **原始实现的问题不是设计，而是初始化**
   - 设计是正确的：trap entry 时从 TrapFrame 加载 cpu_ptr
   - 问题是某些 TrapFrame 的 cpu_ptr 未正确初始化

### 下一步

需要重新实现 TLS 支持，确保：
1. 恢复 TrapFrame.cpu_ptr 字段
2. 恢复 trap_entry.S 中的 tp 加载逻辑
3. 彻底排查所有创建 TrapFrame 的位置，确保 cpu_ptr 正确初始化
4. 特别关注：
   - TaskStruct 创建时
   - fork/clone 时
   - exec 时
   - 内核线程创建时

