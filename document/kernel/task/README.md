# 任务子系统设计

任务子系统定义内核的运行单元,生命周期和阻塞/唤醒边界.字段细节和具体 syscall 分支请看源码与 rustdoc.

## 当前状态

Comix 使用统一的 `Task` 表示进程,线程和内核线程.进程是 `pid == tid` 的线程组 leader, 线程与 leader 共享部分资源, 内核线程没有用户地址空间.所有任务通过 `SharedTask = Arc<SpinLock<Task>>` 在调度器,任务管理器,wait queue,信号和文件系统之间传递.

当前任务模型已经支持:

- 每任务独立内核栈,`TrapFrame` 和调度 `Context`.
- 用户任务与内核线程共用第一次调度入口 `forkret`.
- `execve` 替换用户地址空间,重建用户栈和 `TrapFrame`.
- `TASK_MANAGER` 维护全局 tid 映射,父子关系查询和退出状态.
- 调度器负责运行状态转换和 run queue 维护.

## 目标和非目标

目标:

- 用一个任务模型承载进程,线程和内核线程.
- 将任务身份/资源生命周期与调度队列状态分开维护.
- 让 clone/fork/exec/exit/wait 共享清晰的资源所有权约束.
- 通过 `Arc` 表达线程共享资源, 通过独立内核栈和上下文表达可调度实体.

非目标:

- 不在文档中维护 `Task` 字段大全.
- 不把 Linux 完整调度策略或完整线程组语义放进当前模型.
- 不在任务模块直接实现具体架构的 trap 保存格式.

## 模块边界

- `task_struct.rs`: 任务对象,共享资源引用,创建和 exec 上下文重建.
- `task_manager.rs`: tid 分配,全局任务表,退出状态和任务查询.
- `process.rs`,`ktask.rs`: 用户进程/内核线程创建和进程级操作.
- `scheduler/*`: run queue,状态迁移,CPU 选择和上下文切换计划.
- `arch/*/kernel/task.rs`: 架构 ABI 相关的用户栈和返回准备.

任务模块可以持有文件,地址空间,信号和凭证对象的引用, 但这些对象的内部规则由各自子系统负责.

## 生命周期

```text
create
  -> Running and queued
  -> scheduled on CPU
  -> running in kernel or user mode
  -> sleep on event
  -> wake and queued again
  -> exit to Zombie
  -> parent wait or release removes global reference
```

`Running` 同时表示"正在 CPU 上运行"或"可运行并在队列中等待".当前模型没有单独的 Ready 状态, 是否已经入队由调度器队列和 `on_cpu` 辅助表达.

退出路径分两层:

- 任务管理器写入退出码并维护全局任务表.
- 调度器把任务状态切到 `Zombie` 并从 run queue 移除.

进程级退出会先切回全局内核页表, 再释放用户地址空间,关闭 fd,分离 SysV shared memory.线程退出会释放自己对共享地址空间的引用, 最后一个引用由 `Arc` 生命周期回收.

## 并发和生命周期约束

- `Task` 内部由 `SpinLock` 保护, 不要长期持有任务锁后再调用可能唤醒或调度的路径.
- `TASK_MANAGER` 负责全局可见性, 但不拥有调度状态转换.
- `trap_frame_ptr` 指向任务私有保存区, 同一任务不能被两个 CPU 同时调度运行.
- 多核唤醒必须幂等: 已经是 `Running`,`Zombie` 或 `Stopped` 的任务不能重复入队.
- `execve` 写用户栈前必须确保新地址空间已可访问, 否则会在内核态触发页错误.

## 已知限制

- `Task` 仍混合了调度热字段和进程资源字段, 后续可拆分为更清晰的运行态/资源态结构.
- `preempt_count` 和通用优先级字段还没有形成完整内核抢占模型.
- 线程用户栈隔离由调用方和 clone 参数保证, 任务结构本身不自动分配用户线程栈.

## 文档导航

- [任务结构](task.md): `Task` 模型, 身份关系和资源引用边界.
- [调度器](scheduler.md): per-CPU RR 调度器, CPU 选择和唤醒幂等性.
- [上下文切换](context.md): `Context` 与架构切换边界.
- [内存空间](memory_space.md): 任务和 `MemorySpace` 的生命周期关系.
- [等待队列](wait_queue.md): sleep/wake 约束和阻塞路径.

## 源码索引

- `os/src/kernel/task/task_struct.rs`: `Task`,`SharedTask`,创建,exec 和资源引用.
- `os/src/kernel/task/task_manager.rs`: `TASK_MANAGER`,tid 分配,退出码和全局查询.
- `os/src/kernel/task/mod.rs`: `forkret`,当前任务访问,进程退出资源清理.
- `os/src/kernel/boot.rs`: PID 1,kthreadd 和 per-CPU idle task 创建.
- `os/src/kernel/syscall/task/*`: fork/clone/exec/exit/wait 等 syscall 入口.
