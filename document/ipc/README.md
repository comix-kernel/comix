# IPC 子系统概述

## 简介

IPC（进程间通信）为 Task 之间传递数据、事件与共享状态提供标准机制。本子系统目前包含四类能力：
- 管道（Pipe）：基于内核缓冲区的字节流通信，适合一写一读或少量端点的单机通信。
- 消息（Message）：以离散消息为单位的传递机制，适合结构化、边界明确的通信。
- 共享内存（Shared Memory）：多任务映射同一物理页，实现零拷贝数据共享。
- 信号（Signal）：面向事件/控制流的异步通知与中断唤醒。

## 设计目标
- 统一抽象：各模块在接口与错误语义上尽量对齐，便于组合使用。
- 高效与可预期：常见路径零拷贝（共享内存）、有界缓冲（管道/消息）、明确的阻塞/非阻塞行为。
- 与内核其他子系统良好耦合：调度器、等待队列、VFS、内存管理。

## 与其他子系统的交互
- 调度与等待：阻塞型 API 通过 `WaitQueue` 与调度器配合实现睡眠与唤醒。
- VFS：管道以文件的形式出现在 VFS 中（`vfs/impls/pipe_file.rs`），可被 `fd_table` 引用。
- 内存管理：共享内存通过 `mm` 为多个 `MemorySpace` 建立映射。
- 任务/信号：信号可打断可中断的睡眠，并作为错误返回（如 `EINTR`）或触发默认动作。

## 源码导览
- IPC 模块根：`os/src/ipc/mod.rs`
- 管道：`os/src/ipc/pipe.rs`（与 `os/src/vfs/impls/pipe_file.rs` 协作）
- 消息：`os/src/ipc/message.rs`
- 共享内存：`os/src/ipc/shared_memory.rs`
- 信号：`os/src/ipc/signal.rs`

## 导航
- [管道（Pipe）](./pipe.md)
- [消息（Message）](./message.md)
- [共享内存（Shared Memory）](./shared_memory.md)
- [信号（Signal）](./signal.md)
- [信号生命周期](./signal_lifecycle.md)