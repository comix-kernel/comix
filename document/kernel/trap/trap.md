# 中断处理模块概述

本文档描述了 `comix` 内核如何处理来自硬件的异常、中断和系统调用，这一整套机制统称为中断（Trap）。

## 1. 什么是中断 (Trap)？

在 RISC-V 架构中，任何导致正常指令流被意外打断的事件都称为一个 Trap。它主要分为三类：

1.  **异常 (Exception)**: 在执行指令时由内部事件引发，例如访问了无效的内存地址（页错误）、执行了非法指令等。这是同步事件。
2.  **中断 (Interrupt)**: 由外部设备异步引发的事件，例如时钟中断、I/O设备中断等。
3.  **系统调用 (System Call)**: 由用户态程序通过 `ecall` 指令主动触发，请求内核服务的事件。

当一个 Trap 发生时，CPU硬件会自动暂停当前执行流，并将控制权转移给内核预设的中断处理程序。

## 2. 初始化

为了让内核能够响应中断，必须在启动阶段进行初始化。

**源码链接**: [`os/src/arch/riscv/trap/mod.rs`](/os/src/arch/riscv/trap/mod.rs)

初始化函数 `init()` 执行以下关键操作：

1.  **设置中断向量**:
    - 将 `stvec` 寄存器的值设置为汇编函数 `trap_entry` 的地址。`stvec` (Supervisor Trap Vector Base Address Register) 告诉 CPU 在S模式下发生 Trap 时应该跳转到哪里。
2.  **使能中断**:
    - 通过 `sie` (Supervisor Interrupt Enable) 寄存器，使能内核需要处理的几类中断，主要是外部中断（`SEIE`）、时钟中断（`STIE`）和软件中断（`SSIE`）。

## 3. 中断处理流程

一次完整的中断处理和返回流程可以分为三个阶段：进入、处理和返回。

### 3.1. 进入中断 (`trap_entry`)

当 Trap 发生时，硬件完成初步状态保存后，会立即跳转到 `stvec` 指向的 `trap_entry` 函数。这是一个汇编实现的底层入口。

**源码链接**: [`os/src/arch/riscv/trap/trap_entry.S`](/os/src/arch/riscv/trap/trap_entry.S)

`trap_entry` 的核心职责是 **保存完整的CPU上下文**：

1.  **准备栈空间**: 它首先在当前任务的内核栈上分配一块空间，用于存放 `TrapFrame`。
2.  **交换 `sscratch`**: 使用 `csrrw` 指令，将 `sscratch` 寄存器（通常预先保存了指向 `TrapFrame` 的指针）与一个通用寄存器（如 `a0`）交换，以便在不破坏任何寄存器的情况下安全地访问 `TrapFrame`。
3.  **保存通用寄存器**: 将全部32个通用寄存器（`x0`-`x31`）的值保存到内核栈上的 `TrapFrame` 结构中。
4.  **保存 CSR**: 将 `sstatus` 和 `sepc` 这两个关键的控制状态寄存器（CSR）的值也保存到 `TrapFrame` 中。
5.  **调用 Rust 处理函数**: 在所有上下文都安全保存后，它会调用 Rust 实现的 `trap_handler` 函数，并将指向 `TrapFrame` 的指针作为参数传递过去。

### 3.2. 中断分发 (`trap_handler`)

`trap_handler` 是用 Rust 实现的高层中断处理函数，负责根据中断原因进行分发。

**源码链接**: [`os/src/arch/riscv/trap/trap_handler.rs`](/os/src/arch/riscv/trap/trap_handler.rs)

其工作流程如下：

1.  **识别中断原因**: 读取 `scause` 寄存器，判断 Trap 的类型（是中断还是异常）和具体原因码。
2.  **分发处理**:
    - **系统调用**: 如果 `scause` 表明是来自用户态的 `ecall`，则调用 `syscall()` 函数处理系统调用。`TrapFrame` 中的 `a7` 寄存器存放系统调用号，`a0`-`a6` 存放参数。
    - **时钟中断**: 如果是时钟中断，则调用 `timer_tick()`，这会触发调度器的 `update_time_slice`，可能导致任务抢占。
    - **页错误**: 如果是访存异常（`LoadPageFault`, `StorePageFault`），则调用相应的页错误处理函数。
    - **其他异常/中断**: 根据 `scause` 的值，分发到对应的处理逻辑。如果遇到无法处理的异常，则会触发 `panic`。
3.  **返回**: 处理完成后，`trap_handler` 函数返回，控制权回到 `trap_entry.S`。

### 3.3. 返回中断 (`__restore`)

当 `trap_handler` 返回后，汇编代码会跳转到 `__restore` 标签处，开始执行中断返回流程。

**源码链接**: [`os/src/arch/riscv/trap/trap_entry.S`](/os/src/arch/riscv/trap/trap_entry.S)

`__restore` 的职责与 `trap_entry` 相反，它负责 **恢复完整的CPU上下文**：

1.  **恢复 CSR**: 从 `TrapFrame` 中加载 `sstatus` 和 `sepc` 的值，并写回对应的物理寄存器。
2.  **恢复通用寄存器**: 从 `TrapFrame` 中将 `x0`-`x31` 的值依次加载回 CPU 的通用寄存器。
3.  **执行 `sret`**: 最后，执行 `sret` (Supervisor Return) 指令。这条指令是原子操作，它会：
    - 将 `pc` (程序计数器) 的值设置为 `sepc` 寄存器的值。
    - 根据 `sstatus` 中的 `SPP` 位恢复到之前的特权级（S模式或U模式）。
    - 根据 `sstatus` 中的 `SPIE` 位恢复中断使能状态。

至此，CPU 的状态完全恢复到 Trap 发生前的样子，任务得以从被中断的地方继续无缝执行。

## 4. 关键数据结构：`TrapFrame`

`TrapFrame` 是整个中断处理机制的基石。它是一个定义在 Rust 中的结构体，其内存布局 **必须** 与 `trap_entry.S` 中保存和恢复寄存器的顺序严格一致。

**源码链接**: [`os/src/arch/riscv/trap/mod.rs`](/os/src/arch/riscv/trap/mod.rs)

它包含了所有通用寄存器以及 `sstatus`、`sepc` 等关键信息，是任务在被中断那一刻的完整快照。通过在 `trap_handler` 中修改 `TrapFrame` 的内容（例如，修改 `sepc` 来改变返回地址，或修改 `a0` 来设置系统调用的返回值），内核可以精确地控制任务恢复执行时的状态。