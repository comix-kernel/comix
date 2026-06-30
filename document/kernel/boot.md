# 内核启动设计

本文描述架构入口交给通用内核后的启动模型.具体初始化函数参数和错误分支以源码为准.

## 当前状态

启动流程已经从架构目录收敛到 `os/src/kernel/boot.rs`.RISC-V 和 LoongArch 只提供 `PrimaryBootOps` hook, 公共代码负责 BSS 清零,内存管理,trap/platform/time/timer 初始化,idle task 建立,PID 1 创建和进入 `/sbin/init`.

RISC-V 在公共启动中插入 CPU 指针初始化和从核启动.LoongArch 当前插入基础 FPU 使能, 多核启动尚未接入.

## 目标和非目标

目标:

- 让两个架构共享同一条内核启动主线.
- 保证每个在线 CPU 都有 `Cpu` 结构,当前任务,当前地址空间和 idle task.
- 在启用中断前完成 trap,timer 和 PID 1 的最小可调度状态.
- 把 rootfs,网络默认接口和用户态 init 放到 PID 1 中完成, 避免早期启动路径继续膨胀.

非目标:

- 不在架构启动代码里复制 `rest_init` 或用户态初始化.
- 不在正式文档中列出每个设备驱动的初始化细节.
- 不保证所有架构具有相同 SMP 能力; 能力差异应由架构 hook 和限制说明表达.

## 模块边界

- 架构入口: 设置机器状态, 然后调用 `kernel::boot::run_primary_boot`.
- `PrimaryBootOps`: 暴露少量有序 hook, 用于 BSS 前后,MM 初始化后,time 初始化后.
- `run_primary_boot`: 维护主核公共启动顺序.
- `rest_init`: 创建 PID 1 并放入 CPU0 调度队列.
- `init`: 作为 PID 1 运行, 创建 `kthreadd`, 初始化 rootfs/网络, 最后执行 `/sbin/init`.
- `create_idle_task`: 为指定 CPU 创建不进入普通运行队列的 idle 任务.

## 主核流程

```text
arch::boot::main
  -> before_clear_bss
  -> clear_bss
  -> after_clear_bss
  -> early tests and boot log
  -> mm::init
  -> after_mm_init
  -> switch to kernel address space
  -> init_boot_trap
  -> platform::init
  -> time::init
  -> after_time_init
  -> timer::init
  -> create CPU0 idle task
  -> trap::init
  -> rest_init
  -> enable interrupts
  -> idle_loop
```

`rest_init` 创建的 PID 1 初始仍是内核任务形态, 第一次被调度后进入 `init`, 再通过 `kernel_execve("/sbin/init")` 变成用户态 init.这样可以在完整调度,trap 和文件系统上下文中完成剩余初始化.

## 从核流程

RISC-V 从核通过 SBI HSM 启动到 `secondary_start`.从核建立自己的 `Cpu` 指针,idle task,全局内核地址空间,trap 和 timer, 然后启用中断进入 idle loop.主核用在线位图等待从核上线, 超时后按实际上线数量继续运行.

LoongArch 当前没有对应的多核 bringup 流程, `num_cpu` 保持单核语义.

## 并发和生命周期约束

- `current_cpu().switch_space` 和 `current_cpu().switch_task` 必须在不可迁移区域内执行.
- idle task 是每 CPU 生命周期资源, 不进入普通 run queue, 只在没有可运行任务时作为兜底上下文.
- PID 1 必须固定为 `tid == pid == 1`, 否则用户态 init 语义会出错.
- 从核上线前只能依赖全局内核页表和 per-CPU idle; 不能假设用户任务已经可迁移到该 CPU.

## 已知限制

- LoongArch 尚无 SMP 启动和 IPI 唤醒.
- `idle_loop` 当前执行一次架构 halt 后依赖中断返回路径继续调度, 不是复杂的电源管理循环.
- `init` 中 rootfs 和网络初始化失败时只记录警告并继续, 便于 bringup, 但不是最终的生产级启动策略.

## 源码索引

- `os/src/kernel/boot.rs`: 公共启动流,PID 1,kthreadd 和 idle task.
- `os/src/arch/riscv/boot/mod.rs`: RISC-V CPU 指针,SBI HSM 从核启动和在线等待.
- `os/src/arch/loongarch/boot/mod.rs`: LoongArch 主核入口和基础 FPU 使能 hook.
- `os/src/kernel/cpu.rs`: per-CPU 状态,当前任务,当前地址空间和 idle task.
