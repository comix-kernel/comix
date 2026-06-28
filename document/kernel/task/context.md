# 任务上下文

## 当前状态

Comix 有两类上下文:

- `Context`: 调度切换用的最小上下文, 保存调用约定要求跨函数调用保持的寄存器.
- `TrapFrame`: trap 边界用的完整上下文, 保存异常/中断/系统调用返回所需的寄存器和特权状态.

任务结构同时持有二者.普通 `schedule` 使用 `Context`; syscall,timer,IPI,异常返回使用 `TrapFrame`.

## 设计边界

`Context` 是架构私有布局, 但通过 `arch::kernel::context::Context` 暴露给通用调度器.RISC-V 保存 `ra/sp/s0-s11`; LoongArch 保存 `ra/sp/fp+s0-s8`.调度器只传递旧/新 `Context` 指针, 不解释字段.

`TrapFrame` 由 trap 汇编入口和架构 `HwTrapFrame` 实现解释.通用任务代码只通过跨架构方法初始化内核线程,exec 返回和 clone/fork 返回.

## 第一次运行

新任务的 `Context.ra` 被设置为 `forkret`.任务第一次被调度时, 汇编 `context_switch` 通过恢复 `ra/sp` 进入 `forkret`, 再由 `forkret` 根据任务类型恢复对应 `TrapFrame`:

- 内核线程恢复到内核态入口.
- 用户任务恢复到用户态入口和用户栈.

## 并发和生命周期约束

- `Context` 指针在切换期间必须指向仍然存活的任务对象.
- `TrapFrame` 指针属于任务私有保存区, 任务被迁移到其他 CPU 后需要更新其中的 CPU 指针.
- trap handler 中发生调度后, 返回时必须恢复当前任务的 `TrapFrame`, 不能盲目恢复入口参数.

## 源码索引

- `os/src/arch/riscv/kernel/context.rs`: RISC-V `Context`.
- `os/src/arch/loongarch/kernel/context.rs`: LoongArch `TaskContext`.
- `os/src/arch/riscv/kernel/switch.S`: RISC-V 上下文切换汇编.
- `os/src/arch/loongarch/kernel/switch.S`: LoongArch 上下文切换汇编.
- `os/src/kernel/task/mod.rs`: `forkret`.
- `os/src/kernel/scheduler/rr_scheduler.rs`: 创建 `SwitchPlan`.
