# TrapFrame 与 Context

## 当前状态

内核使用 `TrapFrame` 和 `Context` 两种上下文保存格式:

- `TrapFrame`: 完整 trap 现场, 用于 syscall,异常,中断和信号返回.
- `Context`: 调度现场, 用于 `schedule` 选择任务后的普通上下文切换.

二者服务不同边界.`TrapFrame` 由 trap entry/restore 使用, 需要精确匹配汇编布局; `Context` 由 `context_switch` 使用, 只保存调用约定要求的最小状态.

## 关键区别

| 项目 | TrapFrame | Context |
| --- | --- | --- |
| 触发边界 | 中断,异常,系统调用 | 调度器切换任务 |
| 保存内容 | 用户/内核返回所需的完整寄存器和特权状态 | callee-saved 寄存器,sp,ra |
| 所属模块 | `arch/*/trap` | `arch/*/kernel` |
| 通用代码视角 | 通过 `HwTrapFrame` 操作 | 只传递指针给 `context_switch` |

## 生命周期

每个任务持有一个 `TrapFrame` 保存区和一个 `Context`.创建任务时先初始化 `Context` 让第一次切换进入 `forkret`; 之后 `forkret` 根据 `TrapFrame` 恢复到内核线程入口或用户态入口.

当 timer 或 IPI 在 trap handler 中触发调度时, 当前 CPU 的任务可能已经改变.trap 返回阶段必须重新从当前任务读取 `TrapFrame`, 否则会恢复到旧任务.

## 源码索引

- `os/src/kernel/task/task_struct.rs`: 任务持有 `Context` 和 `trap_frame_ptr`.
- `os/src/kernel/task/mod.rs`: `forkret` 和当前任务访问.
- `os/src/arch/riscv/trap/trap_frame.rs`: RISC-V `TrapFrame`.
- `os/src/arch/loongarch/trap/trap_frame.rs`: LoongArch `TrapFrame`.
- `os/src/arch/riscv/kernel/context.rs`: RISC-V `Context`.
- `os/src/arch/loongarch/kernel/context.rs`: LoongArch `TaskContext`.
