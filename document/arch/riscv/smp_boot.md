# RISC-V SMP 启动设计

## 当前状态

RISC-V 使用 SBI HSM 启动从核.主核完成公共内核初始化后, 在 time 初始化之后启动从核; 从核建立自己的 per-CPU 状态,idle task,trap 和 timer, 然后进入 idle loop 等待调度.

当前调度器是 per-CPU 运行队列.任务唤醒时可以按 affinity 选择目标 CPU, 跨核唤醒通过 reschedule IPI 通知目标 CPU.

## 目标和非目标

目标:

- 让所有在线 hart 都拥有独立 `Cpu` 状态和 idle task.
- 使用统一内核页表作为从核进入完整内核后的地址空间.
- 用在线位图向主核确认从核上线结果.
- 为 per-CPU 调度器和 IPI 唤醒提供启动基础.

非目标:

- 不在启动阶段做复杂负载均衡.
- 不假设所有请求启动的 hart 都一定成功上线.
- 不在 RISC-V 文档中描述 LoongArch SMP 行为.

## 主核流程

```text
riscv entry.S
  -> riscv::boot::main
  -> setup temporary tp
  -> kernel::boot::run_primary_boot
  -> mm::init
  -> setup CPU0 tp
  -> time::init
  -> boot_secondaries
  -> timer/trap/rest_init
```

`tp` 在 MM 初始化前先指向一个临时值, MM 初始化后再指向 `CPUS[0]`.这保证公共代码通过 `current_cpu` 访问 per-CPU 数据时有稳定基础.

## 从核流程

```text
SBI HSM secondary_sbi_entry
  -> secondary_start
  -> init boot trap
  -> set tp to CPUS[hartid]
  -> mark online
  -> create idle task
  -> switch to global kernel space
  -> trap::init
  -> timer::init
  -> enable interrupts
  -> idle_loop
```

主核为每个目标 hart 调用 SBI HSM, 然后等待 `CPU_ONLINE_MASK` 达到预期值.超时不会阻塞系统启动, 内核会按实际上线 CPU 数继续运行.

## 并发和生命周期约束

- 从核上线后必须先设置 `tp`, 再访问 `current_cpu`.
- 从核的第一个任务必须是本 CPU idle task.
- 从核切到全局内核页表后才能进入完整 trap/timer 运行期.
- 在线 CPU 数由实际上线位图决定, 不是单纯由配置上限决定.

## 已知限制

- 启动等待使用固定超时, 没有更复杂的错误恢复.
- 从核上线后先进入 idle, 任务迁移依赖后续 wakeup 和 IPI.
- CPU hotplug 不在当前设计范围内.

## 源码索引

- `os/src/arch/riscv/boot/mod.rs`: 主核 hook,SBI HSM 启动,`secondary_start` 和在线位图.
- `os/src/kernel/boot.rs`: 公共启动,idle task 创建.
- `os/src/kernel/cpu.rs`: `CPUS` 和 per-CPU 状态.
- `os/src/kernel/scheduler/mod.rs`: per-CPU 调度器和 CPU 选择.
- `os/src/arch/riscv/ipi.rs`: reschedule IPI.
