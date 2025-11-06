# 任务的上下文

本文档解释了 `Task` 结构中的 `context` 字段，以及它在任务调度和上下文切换中的核心作用。

## 1. 什么是任务上下文？

任务上下文（`Context`）是一个数据结构，它保存了任务在被切换出去时，为了能在未来被准确无误地恢复执行所需要保存的最小CPU状态。进一步的描述见[执行上下文](../trap/context.md)

在 `comix` 中，`Context` 主要用于 **非中断驱动的上下文切换**，例如任务主动调用 `yield_task()` 或因时间片用完而被调度器切换。

**源码链接**:
- `Context` 结构体: [`os/src/arch/riscv/kernel/context.rs`](/os/src/arch/riscv/kernel/context.rs)
- 切换汇编代码: [`os/src/arch/riscv/kernel/switch.S`](/os/src/arch/riscv/kernel/switch.S)

## 2. `Context` 结构的设计

```rust
// os/src/arch/riscv/kernel/context.rs
#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct Context {
    pub ra: usize,
    pub sp: usize,
    s: [usize; 12], // s0..s11
}
```

`Context` 的设计遵循了 RISC-V 调用约定，只保存 **被调用者保存（callee-saved）** 的寄存器。

- `ra` (Return Address): 返回地址寄存器。对于 `__switch` 函数来说，它保存了调用 `__switch` 的函数的返回地址。
- `sp` (Stack Pointer): 栈指针寄存器。指向当前任务的内核栈顶。
- `s0` - `s11`: Callee-saved 寄存器。调用约定规定，如果一个函数（被调用者）要使用这些寄存器，它必须在返回前将它们恢复到调用前的状态。因此，在任务切换时，我们必须为任务保存这些寄存器的值。

**为什么不保存所有寄存器？**

- 调用者保存（caller-saved）的寄存器（如 `a0-a7`, `t0-t6`）由调用者负责保存。因为 `schedule()` 函数调用了 `__switch`，编译器生成的代码已经确保了在调用 `__switch` 前后，这些寄存器的值对于 `schedule()` 函数来说是正确的。因此，`__switch` 无需为任务保存它们。
- 这种设计使得 `Context` 结构更小，上下文切换更快。当中断发生时，所有寄存器都会被保存在 `TrapFrame` 中，那是一个更完整的上下文。

## 3. 上下文切换流程 (`__switch`)

当 `schedule()` 函数决定进行任务切换时，它会调用汇编函数 `__switch(old_ctx_ptr, new_ctx_ptr)`。

1.  **保存旧任务上下文**:
    - `__switch` 将 `ra` 和 `sp` 寄存器的当前值保存到 `old_ctx_ptr` 指向的 `Context` 结构中。
    - 接着，它将 `s0` 到 `s11` 这12个 callee-saved 寄存器的值也依次保存到 `Context` 的 `s` 数组中。

2.  **恢复新任务上下文**:
    - `__switch` 从 `new_ctx_ptr` 指向的 `Context` 结构中，将之前为新任务保存的 `ra`, `sp`, `s0-s11` 的值加载回 CPU 的物理寄存器。

3.  **返回并切换执行流**:
    - `__switch` 的最后一条指令是 `ret`。
    - `ret` 指令会将 `ra` 寄存器中的值加载到程序计数器 `pc` 中。由于 `ra` 刚刚从新任务的 `Context` 中恢复，CPU 的执行流便无缝地切换到了新任务上次被切走的地方。
    - **对于一个新任务**，它的 `ra` 在创建时被初始化为 `forkret` 函数的地址。因此，新任务第一次被调度时，会从 `forkret` 开始执行，并最终通过 `sret` 进入任务的真正入口。

这个过程精确地完成了 CPU 核心状态的交接，实现了任务的平滑切换。