# IPC 子系统概述

IPC 子系统为任务之间传递字节流, 离散消息, 共享页和异步事件提供内核侧基础设施。当前实现不是单一框架, 而是一组和 VFS, Task, MemorySpace, syscall 层协作的机制。

## 当前状态

- Pipe: 字节流端点由 VFS `PipeFile` 暴露为 fd, 底层复用 `ipc::Pipe` 和环形缓冲区。
- Message: 内核内消息队列, 以消息类型和 payload 为边界, 通过等待队列提供阻塞收发。
- SysV shared memory: 全局 segment registry 管理 `shmid/key`, syscall 层把 segment 映射进当前 `MemorySpace`。
- Signal: 任务私有 pending 和线程组共享 pending 共同决定返回用户态前的投递行为。

## 目标

- 让 IPC 对外表现为清晰的内核对象生命周期, 而不是把实现结构泄漏给 syscall 或应用层。
- 让等待, 唤醒和信号中断语义集中在内核调度边界处理。
- 让共享内存的数据面通过页映射共享, 控制面通过 registry 和 attachment table 管理。

## 非目标

- 不在文档中维护完整 API/字段清单, 具体参数和错误分支以 rustdoc 和源码为准。
- 不承诺完整 Linux System V IPC 全集。当前正式落地的是 SysV shm, 消息队列仍是内核内基础队列。
- 不把管道, socket, 消息队列抽象成统一传输层。

## 模块边界

- `os/src/ipc/`: IPC 核心对象和共享状态。
- `os/src/kernel/syscall/ipc.rs`: pipe/dup 和 SysV shm syscall 的 ABI 边界。
- `os/src/vfs/impls/pipe_file.rs`: pipe 的 fd/File 语义。
- `os/src/kernel/task/`: exit, exec, clone 中和 IPC 相关的资源复制或清理。
- `os/src/kernel/syscall/signal.rs`: 信号 syscall 和用户态信号栈恢复。

## 关键流程

1. 用户态通过 syscall 进入 ABI 层。
2. syscall 层完成 fd 查找, 用户指针复制, flag 校验和 errno 映射。
3. IPC 核心对象只维护内核状态, 如 ring buffer, message queue, shm registry, pending signal。
4. 阻塞路径交给调度器或等待队列, 可被可投递信号中断。
5. 进程退出或 exec 时由 task 清理路径关闭 fd, 分离 shm, 释放地址空间。

## 并发和生命周期约束

- IPC 对象通常由 `Arc` 持有, 生命周期由 fd table, task attachment table 或全局 registry 共同决定。
- 等待路径必须避免在持有长生命周期锁时睡眠。
- SysV shm 的 registry 锁, MemorySpace 锁和 task 锁不能长期嵌套持有。
- Signal 投递只标记 pending, 真正处理发生在安全检查点。

## 已知限制

- MessageQueue 当前是内核内队列, 没有完整 msgget/msgsnd/msgrcv/msgctl ABI。
- Pipe 的阻塞和 poll 语义主要在 VFS `PipeFile` 中体现, `ipc::Pipe` 本身保持为底层字节缓冲对象。
- Signal 尚未完整实现 SA_RESTART 和实时信号队列。

## 源码索引

- `os/src/ipc/mod.rs`: IPC 模块导出入口。
- `os/src/ipc/pipe.rs`: pipe 底层字节缓冲对象。
- `os/src/ipc/message.rs`: 内核消息队列。
- `os/src/ipc/shared_memory.rs`: SysV shm segment registry。
- `os/src/ipc/signal.rs`: pending signal, 默认动作和用户态 handler 安装。
- `os/src/kernel/syscall/ipc.rs`: pipe2, dup, shmget/shmat/shmdt/shmctl。
- `os/src/kernel/task/mod.rs`: exit/exec 的 shm detach 和进程资源清理。

## 导航

- [管道](./pipe.md)
- [消息](./message.md)
- [共享内存](./shared_memory.md)
- [信号](./signal.md)
- [信号生命周期](./signal_lifecycle.md)
