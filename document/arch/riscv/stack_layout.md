# 用户 exec 栈布局

本文位于 RISC-V 文档目录是历史原因.当前设计通过 `arch::task::ExecStackLayout` 抽象为跨架构接口, RISC-V 和 LoongArch 分别实现自己的用户栈和 TLS 细节.

## 当前状态

`Task::execve` 调用架构 `setup_exec_stack_layout`, 得到初始用户栈指针,`argc`,`argv`,`envp` 和可选 TLS/thread pointer.随后通过 `HwTrapFrame::set_exec_trap_frame_from_layout` 写入架构 `TrapFrame`.

共同语义:

- 用户栈向低地址增长.
- `argv` 和 `envp` 字符串以 NUL 结尾.
- 指针数组以 NULL 结尾.
- 最终栈指针满足 ABI 对齐要求.
- auxv 至少提供动态链接器和 libc 启动所需的基本条目.

RISC-V 当前不设置 TLS 值, LoongArch 会在用户栈顶部预留 TLS/TCB 区域并设置 `$tp`.

## 目标和非目标

目标:

- 为 `/sbin/init`,busybox 和动态链接器提供足够的 Linux ABI 启动栈.
- 把 RISC-V 和 LoongArch 的寄存器差异限制在各自 `kernel/task.rs` 和 `trap_frame.rs` 中.
- 让通用 exec 路径只处理 `ExecStackLayout`, 不解释架构寄存器.

非目标:

- 不在文档中维护 auxv 条目大全.
- 不为每种 libc 变体记录特例.
- 不在正式文档中保留长伪代码.

## 关键流程

```text
exec loader builds MemorySpace
  -> activate or otherwise make user stack writable
  -> arch setup_exec_stack_layout
  -> Task::execve stores new MemorySpace
  -> HwTrapFrame::set_exec_trap_frame_from_layout
  -> forkret_restore returns to user entry
```

栈内容从高地址向低地址写入.实现通常先写字符串和少量平台数据, 再写 auxv,envp,argv 和 argc 区域.通用代码只依赖返回的 `ExecStackLayout`, 不依赖实际排布顺序.

## 并发和生命周期约束

- 写用户栈前必须确保目标用户页已映射且内核可以访问.
- RISC-V 直接写用户栈时需要临时开启 SUM; LoongArch 通过地址空间翻译后写入.
- `argv`,`envp`,auxv 指针必须全部指向新地址空间内的用户地址.
- `TrapFrame` 写入必须在任务私有保存区内完成, 不能复用旧用户现场.

## 已知限制

- auxv 内容是 Linux ABI 子集, 不是完整内核实现.
- RISC-V TLS 仍为空值, 后续线程 TLS 支持需要补齐.
- LoongArch TLS 布局是满足当前 libc 启动的最小实现, 后续可与更完整 ABI 文档对齐.

## 源码索引

- `os/src/arch/task.rs`: `ExecStackLayout` 跨架构返回结构.
- `os/src/kernel/task/task_struct.rs`: `Task::execve` 调用栈布局并写入 `TrapFrame`.
- `os/src/arch/riscv/kernel/task.rs`: RISC-V 用户栈布局.
- `os/src/arch/loongarch/kernel/task.rs`: LoongArch 用户栈和 TLS 布局.
- `os/src/arch/riscv/trap/trap_frame.rs`: RISC-V exec `TrapFrame` 写入.
- `os/src/arch/loongarch/trap/trap_frame.rs`: LoongArch exec `TrapFrame` 写入.
