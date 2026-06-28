# 管道

Pipe 提供单向字节流 IPC。当前设计分成两层: `ipc::Pipe` 负责共享环形缓冲区, VFS `PipeFile` 负责 fd 语义, 阻塞/非阻塞和 poll 边界。

## 当前状态

- `make_pipe()` 创建读端和写端, 两端共享同一个 `PipeRingBuffer`。
- 底层缓冲以字节为单位读写, 不保存消息边界。
- syscall 层的 `pipe2()` 创建 `PipeFile` 对, 放入当前任务 fd table, 再把 fd 写回用户空间。
- `PipeRingBuffer` 保存写端弱引用, 用于判断写端是否已经全部释放。

## 目标

- 把 pipe 作为普通 fd 暴露给 VFS I/O 路径。
- 让底层 IPC 代码只关心字节流和端点生命周期。
- 让用户指针复制, fd 分配, close-on-exec 等策略停留在 syscall/VFS 层。

## 非目标

- `ipc::Pipe` 不直接实现 Linux pipe syscall 的全部错误分支。
- 不在 pipe 层保存应用协议消息边界。
- 不在本文维护 ring buffer 容量和每个 errno 的清单。

## 模块边界

- `os/src/ipc/pipe.rs`: 共享缓冲区和读写端对象。
- `os/src/vfs/impls/pipe_file.rs`: `File` trait, read/write/poll/close 语义。
- `os/src/kernel/syscall/ipc.rs`: `pipe2()` 参数校验和 fd table 写入。
- `os/src/kernel/syscall/io.rs`: 通用 read/write 重试, `WouldBlock`, 信号中断。

## 关键流程

1. `pipe2()` 校验 flag, 创建读端和写端文件对象。
2. fd table 分配两个 fd, 失败时回滚已分配 fd。
3. 读写 syscall 通过 VFS `File` 进入 pipe 文件对象。
4. pipe 文件对象在需要时访问 `ipc::Pipe` 的 ring buffer。
5. 写端全部关闭后, 读侧可根据 VFS 层状态形成 EOF。

## 并发和生命周期约束

- 共享缓冲区由 `Arc<Mutex<PipeRingBuffer>>` 保护。
- `PipeRingBuffer` 使用写端 `Weak<Pipe>` 判断端点释放, 避免写端和缓冲区互相强引用。
- 用户缓冲区复制发生在 syscall/VFS I/O 边界, IPC 层不保存用户指针。
- 阻塞等待必须在释放缓冲区锁后进行。

## 已知限制

- 底层 `ipc::Pipe` 是尽量简单的字节缓冲封装, 复杂语义依赖 VFS pipe 文件实现。
- 文档不承诺固定的 pipe 原子写大小, 以源码和测试为准。

## 源码索引

- `os/src/ipc/pipe.rs`: `Pipe`, `PipeRingBuffer`, `make_pipe()`。
- `os/src/vfs/impls/pipe_file.rs`: pipe 作为 `File` 的行为。
- `os/src/kernel/syscall/ipc.rs`: `pipe2()`。
- `os/src/kernel/syscall/io.rs`: 通用 I/O 等待和信号中断。
