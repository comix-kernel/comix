# Task 模型

## 当前状态

`Task` 是内核唯一的可调度实体.用户进程,用户线程,内核线程和 idle task 都以 `Task` 表示, 差异由地址空间,pid/tid 关系和共享资源引用体现.

基本规则:

- `pid == tid` 表示线程组 leader, 即进程.
- `pid != tid` 表示同一进程内的线程.
- `memory_space == None` 表示内核线程.
- 每个任务都有自己的内核栈,`TrapFrame` 保存区和 `Context`.
- 文件表,信号表,命名空间,资源限制等对象按 clone/创建语义通过 `Arc` 共享或复制.

## 目标和非目标

目标:

- 让调度器只关心 `SharedTask`,状态,CPU 归属和上下文.
- 让进程资源可以随 clone/exec/exit 明确共享,复制或释放.
- 让 trap 返回和普通任务切换都能从任务对象找到自己的保存区.

非目标:

- 不复制 Linux `task_struct` 的完整字段和状态机.
- 不在 `Task` 文档里展开文件系统,信号和内存管理的内部实现.

## 关键流程

### 创建

内核线程和用户任务创建时都会分配内核栈和 `TrapFrame` 保存区, 初始化 `Context.ra = forkret`.第一次被调度时, `forkret` 根据任务是否有用户地址空间选择内核线程恢复或用户态恢复.

### exec

`execve` 替换当前任务的用户地址空间, 处理 `CLOEXEC` fd, 构造 argv/envp/auxv 用户栈, 最后由架构 `HwTrapFrame` 接口重建返回用户态所需的 `TrapFrame`.

### exit

任务退出先写入退出码和状态, 再从 run queue 移除.进程 leader 退出时释放进程级资源并唤醒父任务 wait 路径; 非 leader 线程退出时释放线程自己的引用.

## 并发和生命周期约束

- `Task` 对象被 `Arc` 持有, 从 `TASK_MANAGER` 移除不等于立即析构.
- 当前 CPU 的任务引用必须在不可迁移上下文中读取.
- 地址空间释放前必须确保 CPU 已切到全局内核页表.
- 任务状态的调度可见变化应经过调度器接口, 避免绕过 run queue.

## 源码索引

- `os/src/kernel/task/task_struct.rs`: 任务对象和创建/exec 逻辑.
- `os/src/kernel/task/task_manager.rs`: 全局任务生命周期管理.
- `os/src/kernel/task/process.rs`: 进程级创建,退出和 wait 关系.
- `os/src/kernel/task/ktask.rs`: 内核线程创建.
- `os/src/kernel/task/task_state.rs`: 任务状态定义.
