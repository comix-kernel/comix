# 信号生命周期

本文描述信号从产生到恢复原执行流的设计路径。具体 syscall 参数, 信号编号和错误分支以源码为准。

## 当前状态

信号生命周期分为四段:

- 产生: syscall, trap, 内核事件或 IPC 路径请求发送信号。
- 挂起: 目标任务或线程组的 pending 集合记录信号。
- 投递: 返回用户态前检查未屏蔽信号。
- 恢复: 默认动作直接在内核完成, 用户 handler 通过 `rt_sigreturn` 恢复上下文。

## 目标

- 保证信号只在安全检查点改变用户态执行流。
- 让阻塞 syscall 能被真正需要处理的信号打断。
- 把用户栈帧格式集中在信号投递和 `rt_sigreturn` 两端。

## 非目标

- 不描述所有信号默认行为表。
- 不承诺完整 POSIX/Linux restart semantics。
- 不把信号处理函数视为内核回调, handler 始终在用户态运行。

## 关键流程

### 1. 发送

发送方根据目标 pid/tid/tgid 找到任务, 校验基本参数后把对应 signal bit 放入 pending 集合。进程级信号进入共享 pending, 线程定向信号进入任务私有 pending。

### 2. 等待中断

阻塞 I/O, poll, socket 等路径在让出 CPU 后会检查 pending 信号。只有未被屏蔽且动作不是默认忽略/显式忽略的信号才应让 syscall 返回 `EINTR`。

### 3. 返回用户态前投递

`check_signal()` 读取当前任务状态:

1. 先检查私有 pending。
2. 再检查共享 pending。
3. 过滤 blocked mask。
4. 取编号最小的可投递信号。
5. 根据动作表执行默认动作, 忽略或安装用户态 handler。

### 4. 用户 handler

自定义 handler 需要内核在用户栈上写入 `rt_sigframe`, 保存被打断时的 mcontext 和 sigmask。随后内核修改 trap frame, 让用户态从 handler 入口继续执行。

### 5. sigreturn

handler 结束后进入 `rt_sigreturn` syscall。内核从用户栈读取 ucontext, 恢复 blocked mask 和 trap frame, 再返回原执行点。

## 并发和生命周期约束

- pending 信号只表示有事件待处理, 不等于马上改变执行流。
- 共享 pending 需要在线程组语义下处理, 避免多个线程重复消费同一信号。
- stop/continue 会修改线程组内多个任务状态, 必须避开 Zombie 任务。
- 终止信号会走进程级资源清理, 包括 fd 和 SysV shm detach。

## 已知限制

- 实时信号队列未完整实现。
- `SA_RESTORER` 支持基础兼容, 默认 restorer 依赖架构 trampoline。
- `SA_RESTART` 尚未完整驱动 syscall restart。

## 源码索引

- `os/src/ipc/signal.rs`: `check_signal()`, 默认动作, handler 栈帧安装。
- `os/src/kernel/syscall/signal.rs`: 信号 ABI 和 `rt_sigreturn()`。
- `os/src/kernel/task/mod.rs`: 终止路径的资源清理。
- `os/src/uapi/signal.rs`: `SignalAction`, `RtSigFrame`, `UContextT`。
