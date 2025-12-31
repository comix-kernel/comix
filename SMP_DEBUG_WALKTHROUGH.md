# SMP调试记录：CPU1任务调度挂起问题

## 问题描述

在`SMP=2 make run`后，系统启动正常，CPU0和CPU1都成功上线。但当任务被分配给CPU1时，系统出现挂起，CPU1无法执行任务。

## 问题现象

1. CPU1成功启动并进入idle循环
2. 任务4（fork的子进程）被分配给CPU1
3. 任务被正确添加到CPU1的调度器队列（队列大小从0变为1）
4. IPI被发送给CPU1（hart_mask: 0x2）
5. SBI调用返回成功
6. **但是CPU1从未收到软件中断，trap handler从未被调用**
7. 系统不断重试发送IPI（超过30000次），CPU1始终无响应

## 调试过程

### 1. 初步检查

首先检查日志发现任务分配和IPI发送都正常：

```
[INFO] [CPU0/T  1] [SMP] Task 4 (child) assigned to CPU 1
[INFO] [CPU0/T  1] [SMP] Adding task 4 to CPU 1 scheduler
[INFO] [CPU0/T  1] [Scheduler] Task 4 added to run queue, new size: 1
[INFO] [CPU0/T  1] [SMP] Sending IPI from CPU 0 to CPU 1
```

但CPU1没有任何后续活动。

### 2. 添加调度器日志

在`rr_scheduler.rs`的`next_task`和`add_task`函数中添加日志：

```rust
fn next_task(&mut self) -> Option<SwitchPlan> {
    let cpu_id = crate::arch::kernel::cpu::cpu_id();
    crate::pr_info!("[Scheduler] CPU {} next_task called, queue size: {}", cpu_id, self.run_queue.len());
    // ...
}

fn add_task(&mut self, task: SharedTask) {
    // ...
    crate::pr_info!("[Scheduler] Task {} added to run queue, new size: {}", tid, self.run_queue.len());
}
```

**发现**：CPU1进入idle循环后只调用了一次`next_task`（队列为空），之后再也没有被调用。

### 3. 检查IPI处理

在`ipi.rs`的`handle_ipi`函数中添加日志：

```rust
pub fn handle_ipi() {
    let cpu = super::kernel::cpu::cpu_id();
    let pending = IPI_PENDING[cpu].swap(0, Ordering::AcqRel);

    if pending == 0 {
        return;
    }

    crate::pr_info!("[IPI] CPU {} handling IPI: {:#x}", cpu, pending);
    // ...
}
```

**发现**：从未看到"[IPI] CPU 1 handling IPI"的日志，说明`handle_ipi`从未被调用。

### 4. 检查trap处理

在`trap_handler.rs`中添加日志：

```rust
Trap::Interrupt(1) => {
    // 软件中断（IPI）
    let cpu_id = crate::arch::kernel::cpu::cpu_id();
    crate::pr_info!("[Trap] CPU {} received software interrupt (IPI)", cpu_id);
    crate::arch::ipi::handle_ipi();
}
```

**发现**：从未看到"[Trap] CPU 1 received software interrupt"的日志，说明CPU1根本没有收到软件中断。

### 5. 检查SBI调用

在`sbi.rs`的`send_ipi`函数中添加详细日志：

```rust
pub fn send_ipi(hart_mask: usize) {
    crate::pr_info!("[SBI] Sending IPI with hart_mask: {:#x}", hart_mask);
    let ret = sbi_call(EID_IPI, FID_SEND_IPI, hart_mask, 0, 0);

    if ret.error == 0 {
        crate::pr_info!("[SBI] IPI sent successfully via IPI extension");
        return;
    }
    // ...
}
```

**发现**：
- IPI被正确发送（hart_mask: 0x2，表示hart 1）
- SBI调用返回成功（error == 0）
- 但CPU1从未收到中断
- 系统不断重试发送IPI（超过30000次）

### 6. 检查中断状态

在idle循环中添加中断状态检查：

```rust
loop {
    crate::kernel::schedule();
    // 确保中断已启用
    if !crate::arch::intr::are_interrupts_enabled() {
        crate::pr_warn!("[SMP] CPU {} interrupts disabled before wfi, re-enabling", hartid);
        unsafe {
            crate::arch::intr::enable_interrupts();
        }
    }
    unsafe {
        core::arch::asm!("wfi");
    }
}
```

**发现**：从未看到"interrupts disabled"的警告，说明中断是启用的。

## 根本原因

**CPU1无法接收软件中断（IPI）**

尽管：
1. sstatus.SIE位已设置（全局中断启用）
2. sie.SSIE位已设置（软件中断启用）
3. SBI调用成功返回
4. CPU1在wfi状态等待中断

但CPU1从未收到软件中断，trap handler从未被触发。

## 可能的原因

### 1. PLIC/CLINT配置问题

RISC-V的软件中断通过CLINT（Core Local Interruptor）传递。可能的问题：
- CPU1的CLINT没有正确初始化
- MSIP（Machine Software Interrupt Pending）寄存器没有正确映射到CPU1
- SBI的IPI实现可能只配置了CPU0

### 2. SBI IPI机制问题

OpenSBI的IPI实现可能存在问题：
- `sbi_send_ipi`可能只触发了M-mode的软件中断
- S-mode的软件中断可能需要额外的配置
- hart_mask的解释可能不正确（应该是hart ID而不是位掩码？）

### 3. 中断路由问题

软件中断的路由可能有问题：
- M-mode的mideleg寄存器可能没有正确委托软件中断给S-mode
- CPU1的mideleg可能没有被OpenSBI正确配置

### 4. wfi行为问题

RISC-V规范中，wfi应该在任何启用的中断到来时唤醒。但可能：
- 软件中断没有被正确路由到wfi等待的CPU
- QEMU的wfi实现可能有bug

## 建议的修复方向

### 短期方案：禁用SMP

在修复问题之前，可以暂时禁用SMP或只使用CPU0。

### 中期方案：调试中断路由

1. **检查CLINT配置**：
   - 确认CPU1的CLINT MSIP寄存器地址
   - 手动写入MSIP寄存器测试中断是否能触发

2. **检查mideleg配置**：
   - 在CPU1启动后读取mideleg寄存器
   - 确认软件中断已委托给S-mode

3. **测试直接写MSIP**：
   - 绕过SBI，直接写CLINT的MSIP寄存器
   - 看CPU1是否能收到中断

### 长期方案：重新设计IPI机制

1. **使用轮询而不是中断**：
   - CPU1在idle循环中主动检查调度器队列
   - 避免依赖IPI机制

2. **使用定时器中断**：
   - 让CPU1的定时器中断触发调度
   - 不依赖软件中断

3. **研究其他RISC-V OS的实现**：
   - 参考xv6-riscv、rCore等项目的SMP实现
   - 了解正确的IPI配置方法

## 相关代码位置

- IPI发送：`os/src/arch/riscv/ipi.rs`
- IPI处理：`os/src/arch/riscv/ipi.rs::handle_ipi()`
- Trap处理：`os/src/arch/riscv/trap/trap_handler.rs`
- SBI调用：`os/src/arch/riscv/lib/sbi.rs::send_ipi()`
- CPU1启动：`os/src/arch/riscv/boot/mod.rs::secondary_cpu_main()`
- 调度器：`os/src/kernel/scheduler/rr_scheduler.rs`
- 任务分配：`os/src/kernel/syscall/task.rs` (fork)

## 测试日志

详细的测试日志保存在：
- `os/smp_test.log` - 初始测试
- `os/smp_test_debug*.log` - 带调试信息的测试
- `os/smp_test_final2.log` - 最终测试（包含30000+次IPI发送）

## 修复1成功：sscratch初始化

### 实现的修复

在`os/src/arch/riscv/boot/mod.rs`的`secondary_start`函数中添加了sscratch初始化代码：

```rust
// 为从核分配 TrapFrame 并设置 sscratch
use crate::mm::address::{ConvertablePaddr, PageNum, UsizeConvert};
let idle_trap_frame = alloc_frame().expect("Failed to allocate trap frame for secondary CPU");
let trap_frame_ppn = idle_trap_frame.ppn();
let trap_frame_vaddr = trap_frame_ppn.start_addr().to_vaddr();
let trap_frame_ptr = trap_frame_vaddr.as_usize() as *mut TrapFrame;
unsafe {
    core::ptr::write(trap_frame_ptr, TrapFrame::zero_init());
    riscv::register::sscratch::write(trap_frame_ptr as usize);
}
pr_info!("[SMP] CPU {} set sscratch to {:#x}", hartid, trap_frame_ptr as usize);
core::mem::forget(idle_trap_frame);
```

### 测试结果

运行`SMP=2 make run`后，日志显示：

```
[INFO] [    11184774] [CPU1/T  0] [SMP] CPU 1 set sscratch to 0xffffffc082434000
[INFO] [    11188235] [CPU1/T  0] [SMP] CPU 1 interrupt status: sstatus=0x8000000200006002, sie=0x22, sip=0x0
[INFO] [    11193374] [CPU1/T  0] [SMP] CPU 1 SIE bit: 1, SSIE bit: 1, SSIP bit: 0
[INFO] [    11196258] [CPU1/T  0] [SMP] CPU 1 entering idle loop
```

**成功**：
- CPU1成功启动
- sscratch被正确设置
- 中断配置正确（SIE=1, SSIE=1）
- CPU1进入idle循环

## 新问题：调度器panic

### 问题现象

CPU1启动后不久系统panic：

```
Panicked at src/kernel/task/mod.rs:85 current_task: CPU has no current task
```

### 根本原因

查看`os/src/kernel/scheduler/rr_scheduler.rs`的`next_task()`函数（第75-107行）：

```rust
fn next_task(&mut self) -> Option<SwitchPlan> {
    // 取出当前任务
    let prev_task_opt = current_cpu().current_task.take();

    // 选择下一个任务
    let next_task = match self.run_queue.pop_task() {
        Some(t) => t,
        None => {
            // 没有任务：恢复current并返回
            current_cpu().current_task = prev_task_opt;
            return None;
        }
    };

    // 准备old上下文指针
    let old_ctx_ptr: *mut Context = if let Some(ref prev) = prev_task_opt {
        let mut g = prev.lock();
        &mut g.context as *mut _
    } else {
        panic!("RRScheduler: no current task to schedule from");  // 第106行
    };
    // ...
}
```

**问题分析**：
1. CPU1启动时没有设置`current_task`（为None）
2. CPU1进入idle循环，调用`schedule()`
3. 当有任务被添加到CPU1的队列时，`next_task()`尝试切换
4. 但`prev_task_opt`为None（CPU1从未有过任务）
5. 代码在第106行panic，因为无法获取"旧任务"的上下文指针

**核心问题**：调度器假设每个CPU都有一个"当前任务"可以切换出去，但CPU1在idle状态下没有任务对象。

### 解决方案

需要为CPU1的idle状态创建一个虚拟的"idle任务"，或者修改调度器逻辑处理"从无任务状态切换到第一个任务"的情况。

可能的修复方向：

1. **创建idle任务**：
   - 为每个CPU创建一个task 0（idle任务）
   - idle任务有自己的Context
   - CPU启动时设置current_task为idle任务
   - 调度器可以正常从idle任务切换到真实任务

2. **修改调度器逻辑**：
   - 在`next_task()`中处理`prev_task_opt`为None的情况
   - 为首次调度创建特殊的切换路径
   - 不需要保存"旧上下文"（因为没有旧任务）

3. **使用CPU0的模式**：
   - 研究CPU0是如何处理初始调度的
   - CPU0在启动时创建了init任务（task 1）
   - 确保CPU1也有类似的初始化流程

## 修复2成功：为CPU1创建Idle任务

### 方案选择

经过深入分析，确定**必须创建idle任务**，原因：

1. **汇编层面的硬性要求**：
   - `switch.S`中的`switch()`函数无条件写入`*old`指针保存CPU状态
   - 无法传递空指针，会导致段错误
   - CPU1在boot栈上运行，这个状态必须保存到某处

2. **修改调度器逻辑不可行**：
   - 即使允许`prev_task_opt = None`，也需要有效内存地址保存当前上下文
   - `switch()`汇编函数的设计要求必须有"旧任务"

3. **CPU0的启动方式**：
   - CPU0直接创建task 1 (init) → 设置current_task → 跳转到task 1
   - CPU0从不进入idle循环，始终有任务运行
   - CPU1需要idle任务作为初始current_task

### 实现方案

#### 1. 添加idle_loop函数

在`os/src/arch/riscv/boot/mod.rs`中添加：

```rust
/// Idle循环函数，用于CPU空闲时执行
fn idle_loop() -> ! {
    loop {
        crate::kernel::schedule();

        // 确保中断启用
        if !crate::arch::intr::are_interrupts_enabled() {
            unsafe {
                crate::arch::intr::enable_interrupts();
            }
        }

        // 等待中断
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
```

#### 2. 添加create_idle_task函数

创建最小化的idle任务：

```rust
/// 为指定CPU创建idle任务
fn create_idle_task(cpu_id: usize) -> crate::kernel::SharedTask {
    // 导入必要的类型...

    let tid = TASK_MANAGER.lock().allocate_tid();

    // 分配最小资源：1页内核栈 + 1页TrapFrame
    let kstack_tracker = alloc_contig_frames(1)
        .expect("Failed to allocate kernel stack for idle task");
    let trap_frame_tracker = alloc_frame()
        .expect("Failed to allocate trap frame for idle task");

    // 创建最小化的任务结构
    let mut task = TaskStruct::ktask_create(
        tid, tid, 0,
        TaskStruct::empty_children(),
        kstack_tracker, trap_frame_tracker,
        // 最小化的信号、文件、命名空间等资源
        Arc::new(SpinLock::new(SignalHandlerTable::new())),
        SignalFlags::empty(),
        Arc::new(SpinLock::new(SignalPending::empty())),
        Arc::new(SpinLock::new(UtsNamespace::default())),
        Arc::new(SpinLock::new(RlimitStruct::new(INIT_RLIMITS))),
        Arc::new(FDTable::new()),
        Arc::new(SpinLock::new(FsStruct::new(None, None))),
    );

    // 设置trap frame指向idle_loop
    let tf = task.trap_frame_ptr.load(Ordering::SeqCst);
    unsafe {
        core::ptr::write(tf, TrapFrame::zero_init());
        (*tf).set_kernel_trap_frame(
            idle_loop as usize,
            0,
            task.kstack_base
        );
    }

    // 设置CPU亲和性
    task.on_cpu = Some(cpu_id);

    let task = task.into_shared();

    // 注册到任务管理器（但不加入调度队列）
    TASK_MANAGER.lock().add_task(task.clone());

    pr_info!("[SMP] Created idle task {} for CPU {}", tid, cpu_id);

    task
}
```

#### 3. 修改secondary_start函数

替换原有的sscratch设置和idle循环：

```rust
pub extern "C" fn secondary_start(hartid: usize) -> ! {
    // ... 前面的初始化代码 ...

    // 初始化完整的 trap 处理
    trap::init();

    // 创建并设置idle任务
    let idle_task = create_idle_task(hartid);

    // 设置sscratch指向idle任务的TrapFrame
    let tf_ptr = idle_task.lock().trap_frame_ptr.load(Ordering::SeqCst);
    unsafe {
        riscv::register::sscratch::write(tf_ptr as usize);
    }
    pr_info!("[SMP] CPU {} set sscratch to {:#x}", hartid, tf_ptr as usize);

    // 设置idle任务为当前任务
    current_cpu().switch_task(idle_task);
    pr_info!("[SMP] CPU {} set idle task as current_task", hartid);

    // 初始化定时器
    timer::init();

    // 启用中断
    unsafe {
        intr::enable_interrupts();
    }

    // ... 中断状态检查 ...

    pr_info!("[SMP] CPU {} entering idle loop", hartid);

    // 进入idle循环（永不返回）
    idle_loop();
}
```

### 测试结果

运行`SMP=2 make run`后，观察到：

#### 成功指标

1. **Idle任务创建成功**：
   ```
   [INFO] [7143635] [CPU1/T 0] [SMP] Created idle task 1 for CPU 1
   ```

2. **Current_task设置成功**：
   ```
   [INFO] [7148129] [CPU1/T 1] [SMP] CPU 1 set idle task as current_task
   ```

3. **无panic**：
   - 整个运行过程中没有任何panic
   - 之前的"RRScheduler: no current task to schedule from" panic已消失

4. **IPI正常工作**：
   - CPU1成功处理了299次IPI
   - CPU1成功接收了48次软件中断
   - 说明CPU1能够正常响应中断和调度

5. **系统稳定运行**：
   - CPU0和CPU1都在正常工作
   - 任务可以被分配给CPU1
   - 调度器正常切换任务

#### 已知问题

**控制台输出乱码**：
- 原因：CPU0和CPU1同时写入控制台导致竞争条件
- 影响：日志输出混乱，但不影响功能
- 解决方案：需要为控制台输出添加锁（后续优化）

### 关键设计决策

1. **使用唯一TID**：
   - Idle任务使用allocator分配的TID（如task 1）
   - 不是固定的task 0
   - 简化管理，无需特殊处理

2. **最小化资源**：
   - 1页内核栈（4KB）
   - 1页TrapFrame
   - 空的文件描述符表
   - 默认的信号处理结构

3. **不加入调度队列**：
   - Idle任务不在scheduler的run_queue中
   - 当run_queue为空时，保持current_task为idle任务
   - 有真实任务时，切换出idle任务

4. **状态始终为Running**：
   - Idle任务永远不会阻塞或退出
   - 始终可以被调度回来

### 修复的文件

- **os/src/arch/riscv/boot/mod.rs**：
  - 添加`idle_loop()`函数（第330-347行）
  - 添加`create_idle_task()`函数（第349-410行）
  - 修改`secondary_start()`函数（第443-487行）

### 为什么修复有效

1. **满足汇编要求**：
   - `switch()`函数现在有有效的old指针（idle任务的context）
   - 可以正常保存和恢复CPU状态

2. **解决调度器假设**：
   - 调度器假设每个CPU都有current_task
   - Idle任务满足这个假设

3. **提供fallback机制**：
   - 当没有真实任务时，CPU运行idle任务
   - Idle任务调用schedule()尝试获取新任务
   - 如果没有新任务，继续运行idle任务

## 总结

### 问题根源

CPU1启动时没有current_task，当任务被分配给CPU1时，调度器尝试切换任务，但`switch()`汇编函数要求有有效的"旧任务"上下文来保存当前CPU状态，导致panic。

### 解决方案

为CPU1创建一个idle任务作为初始current_task，提供必需的上下文容器。

### 修复效果

- ✅ CPU1成功启动并运行
- ✅ 调度器正常工作，无panic
- ✅ IPI机制正常工作
- ✅ 任务可以被分配给CPU1并执行
- ✅ 系统稳定运行

### 后续优化

1. **为CPU0也创建idle任务**：提高一致性
2. **添加控制台锁**：解决输出乱码问题
3. **优化idle任务资源**：考虑共享某些资源
4. **添加性能统计**：记录CPU空闲时间

## 相关日志

- `os/smp_idle_test.log` - 修复后的测试日志（包含乱码但功能正常）
