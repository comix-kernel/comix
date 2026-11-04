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

`exec_loader` 会从 `PT_TLS` 取出模板并传给架构栈布局。RISC-V 故意忽略这份模板, `execve` 返回用户态时 `tp` 为 0。静态 glibc 根据 `PT_TLS` 和 auxv 自己建立初始 TCB; 内核提前写入 `tp` 会让早期 pointer guard 看到另一份 TCB。LoongArch 仍在用户栈顶部预留 TLS/TCB, 并在进入用户态前设置 `$tp`。

`clone` 带 `CLONE_SETTLS` 时是另一条路径: 架构 `set_tls` 把用户传入的指针写入子任务 `tp`。RISC-V 写 `x4`, LoongArch 写线程指针寄存器。

RISC-V `execve` 的 `TrapFrame` 把 `a0`/`a1`/`a2` 置 0。`argc`/`argv`/`envp` 只放在用户栈上。静态 glibc 把进入时的 `a0` 当作 `rtld_fini`, 非零值会被当成结束函数。

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
- RISC-V `execve` 不建立初始 TCB, 只保证 `tp` 为 0, 让 glibc 自己完成。内核没有通用的线程 TLS 分配器。
- LoongArch TLS 布局是满足当前 libc 启动的最小实现, 后续可与更完整 ABI 文档对齐.

## 源码索引

- `os/src/arch/task.rs`: `ExecStackLayout` 跨架构返回结构.
- `os/src/kernel/task/task_struct.rs`: `Task::execve` 调用栈布局并写入 `TrapFrame`.
- `os/src/arch/riscv/kernel/task.rs`: RISC-V 用户栈布局.
- `os/src/arch/loongarch/kernel/task.rs`: LoongArch 用户栈和 TLS 布局.
- `os/src/arch/riscv/trap/trap_frame.rs`: RISC-V exec `TrapFrame` 写入.
- `os/src/arch/loongarch/trap/trap_frame.rs`: LoongArch exec `TrapFrame` 写入.
