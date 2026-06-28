# 架构层设计

本文记录 `arch/` 的当前设计边界.具体寄存器编码,页表位和汇编保存槽位以源码与 rustdoc 为准.

## 当前状态

Comix 目前支持 RISC-V64,LoongArch64 和宿主测试用 mock 架构.`os/src/arch/mod.rs` 是架构层唯一公开门面: 目标架构由条件编译选择, 内核其余模块通过 `crate::arch::*`,`ArchImpl`,`PlatformImpl` 和统一的 `TrapFrame` 类型访问架构能力.

RISC-V64 的多核启动,软件中断 IPI,定时器中断和外部中断路径已经接入通用调度.LoongArch64 当前以单核 bringup 和用户态运行为主, 提供同名接口保持通用内核代码可复用, 但 IPI 和外部中断处理仍是后续工作.

## 目标和非目标

目标:

- 把条件编译和寄存器操作限制在 `arch/` 内部.
- 为内核启动,任务切换,trap 返回,地址转换,时钟和 IPI 提供稳定的跨架构接口.
- 允许新架构只实现最小 hook, 复用 `kernel::boot`,调度器和任务模型.
- 保持宿主测试可以通过 mock 架构构建核心模块.

非目标:

- 不在正式文档中维护寄存器字段大全或页表位清单.
- 不把架构私有 crate 或 CSR 操作暴露给 `arch/` 外部模块.
- 不在架构层实现调度策略,任务生命周期或文件系统启动策略.

## 模块边界

- `arch/mod.rs`: 目标架构选择,统一类型别名和便捷包装函数.
- `arch/arch.rs`,`arch/cpu_ops.rs`,`arch/plat.rs`: 定义跨架构 trait 边界.
- `arch/{riscv,loongarch}/boot`: 只负责进入公共启动流前后的架构 hook.
- `arch/{riscv,loongarch}/trap`: 汇编入口,TrapFrame 布局,异常/中断分派和 trap 返回.
- `arch/{riscv,loongarch}/kernel`: 任务上下文和切换汇编.
- `arch/{riscv,loongarch}/mm`: 页表,地址转换和 TLB 相关实现.

通用内核模块不应直接读取 RISC-V CSR 或 LoongArch CSR.确实需要架构行为时, 优先增加 trait 方法或 `arch/mod.rs` 包装.

## 关键流程

### 启动交接

```text
arch entry.S
  -> arch::boot::main
  -> kernel::boot::run_primary_boot
  -> mm/trap/platform/time/timer
  -> idle task
  -> rest_init
  -> /sbin/init
```

架构入口只决定早期 CPU 指针,FPU/CSR 等本架构必须先完成的状态.公共启动顺序由 `run_primary_boot` 维护, 避免两个架构各自复制 init/idle/rootfs 逻辑.

### trap 和任务切换

trap 入口保存完整 `TrapFrame`, Rust handler 分派 syscall,timer,IPI 或设备中断.调度器若在 handler 中切换任务, trap 返回必须恢复"当前任务"的 `TrapFrame`, 而不是入口时传入的旧指针.

普通任务切换使用 `Context`, 只保存调用约定要求的最小寄存器集合.`TrapFrame` 面向异常边界, `Context` 面向调度边界, 二者不能互相替代.

## 并发和生命周期约束

- 访问 `current_cpu()` 前必须处在不可迁移的临界区, 现有代码通常使用 `PreemptGuard`.
- 任务迁移或切换后必须同步 `TrapFrame.cpu_ptr`, 否则 trap entry 恢复内核 `tp` 时可能指向旧 CPU.
- trap handler 运行在硬中断上下文, 不应执行可能阻塞或长期持锁的工作.
- RISC-V IPI 使用 per-CPU 原子 pending 标志, 发送端设置标志后再触发 SBI 软件中断.

## 已知限制

- LoongArch IPI 当前是单核 no-op 接口, 尚未接入多核硬件中断.
- LoongArch trap 当前主要处理 syscall 和 timer; 用户态未知异常仍走 panic/诊断路径, 外部设备中断路径还未与 RISC-V 对齐.
- RISC-V TLB shootdown IPI 已有发送和处理接口, 但更完整的同步等待策略需要由内存管理侧继续收敛.

## 文档导航

- [RISC-V 寄存器速查](riscv/riscv_register.md): RISC-V 常用寄存器和特权态入口背景.
- [RISC-V 用户栈布局](riscv/stack_layout.md): `execve` 后用户栈参数, 环境变量和辅助向量布局.
- [RISC-V 多核启动](riscv/smp_boot.md): 主核/从核启动交接和在线 CPU 管理.
- [RISC-V IPI](riscv/ipi.md): reschedule 和 TLB flush IPI 的当前协议.
- [LoongArch64 状态](loongarch/README.md): LoongArch 当前支持边界.
- [LoongArch bringup 记录](loongarch/bringup_userland.md): LoongArch 用户态启动修复和剩余限制.

## 源码索引

- `os/src/arch/mod.rs`: 架构门面,统一类型和跨架构包装.
- `os/src/arch/arch.rs`: `Arch` 和 `HwTrapFrame` trait 边界.
- `os/src/arch/riscv/boot/mod.rs`: RISC-V 主核 hook,从核启动和在线 CPU 掩码.
- `os/src/arch/loongarch/boot/mod.rs`: LoongArch 主核 hook 和早期 FPU 使能.
- `os/src/arch/riscv/trap/*`: RISC-V trap 入口,分派和恢复.
- `os/src/arch/loongarch/trap/*`: LoongArch trap 入口,分派,TLB refill 入口安装和恢复.
- `os/src/arch/riscv/ipi.rs`: RISC-V IPI pending 标志和 SBI 发送.
- `os/src/arch/loongarch/ipi.rs`: LoongArch 单核 IPI 兼容接口.
