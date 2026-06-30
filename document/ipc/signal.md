# 信号

信号是任务异步事件机制。当前实现把"产生信号"和"处理信号"分离: 发送路径只把信号放入 pending 集合, 返回用户态前的检查点再决定忽略, 默认处理或安装用户态 handler。

## 当前状态

- 每个任务有私有 pending, 线程组共享 pending 和 blocked mask。
- 信号动作表保存默认, 忽略或用户 handler 配置。
- `check_signal()` 在安全点选择第一个未屏蔽 pending 信号处理。
- 默认动作覆盖终止, core dump stub, stop, continue 和 ignore。
- 用户 handler 通过构造 `rt_sigframe` 并修改 trap frame 进入用户态。
- `rt_sigreturn` 从用户栈恢复被信号打断前的上下文。

## 目标

- 让信号成为阻塞 syscall 和任务控制的统一异步事件来源。
- 把用户态 ABI, 如 sigaction/sigprocmask/sigreturn, 放在 syscall 层。
- 把具体架构寄存器恢复封装在 trap frame 抽象之后。

## 非目标

- 当前不实现完整实时信号队列。
- 当前不完整实现 SA_RESTART, 因此部分阻塞 syscall 会直接向用户态暴露 `EINTR`。
- 不在文档列出所有信号编号和默认动作分支。

## 模块边界

- `os/src/ipc/signal.rs`: pending 选择, 默认动作, 用户 handler 栈帧安装。
- `os/src/kernel/syscall/signal.rs`: sigaction, sigprocmask, sigpending, sigtimedwait, sigsuspend, sigreturn。
- `os/src/kernel/task/`: 任务状态, 线程组, exit/stop/continue。
- `os/src/arch/*/trap/`: 返回用户态前的检查点和 trap frame 恢复。

## 关键流程

1. syscall 或内核事件向目标任务/线程组标记 pending。
2. 阻塞等待路径用 `signal_interrupts_syscall()` 判断是否应返回 `EINTR`。
3. 返回用户态前调用 `check_signal()`。
4. 内核从私有 pending 或共享 pending 中找第一个未屏蔽信号。
5. 默认/忽略动作在内核完成, 用户 handler 动作通过修改用户 trap frame 完成投递。
6. 用户 handler 结束后调用 `rt_sigreturn`, 内核恢复保存的上下文和 blocked mask。

## 并发和生命周期约束

- pending 和动作表受 task 内部锁保护。
- 选择可投递信号时会同时考虑 private pending, shared pending 和 blocked mask。
- 安装用户 handler 时必须写用户栈, 失败路径需要谨慎处理, 避免破坏原 trap frame。
- `SIGKILL` 和 `SIGSTOP` 这类不可屏蔽语义不能被普通 mask 延迟。

## 已知限制

- `siginfo_t` 字段只填充基础信息。
- core dump 仍是 stub。
- `SIGCHLD` 等默认忽略信号在 syscall 中断判断上有兼容性特判, 但不是完整 SA_RESTART。

## 源码索引

- `os/src/ipc/signal.rs`: signal pending, 投递, 默认动作。
- `os/src/kernel/syscall/signal.rs`: 信号 syscall。
- `os/src/uapi/signal.rs`: 用户态 ABI 数据结构和常量。
- `os/src/arch/*/trap/`: 信号检查点和 trap frame 恢复。
