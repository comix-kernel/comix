# 任务地址空间设计

## 当前状态

任务通过 `memory_space: Option<Arc<SpinLock<MemorySpace>>>` 关联地址空间.用户任务持有 `Some`, 内核线程持有 `None`.CPU 结构保存当前激活地址空间, 任务切换到用户任务时会激活该任务页表; 内核线程沿用当前 CPU 已有的内核地址空间.

当前采用"用户映射 + 统一内核映射"的共享页表模型.用户进程的地址空间包含自己的用户区域, 同时包含相同的内核高地址映射, 因此 syscall/trap 进入内核时不需要每次切换到独立内核页表.进程退出释放用户地址空间前, 当前 CPU 会先切回全局内核页表.

## 目标和非目标

目标:

- 让用户任务拥有独立用户地址空间.
- 让内核代码在所有用户页表中使用一致的高地址映射.
- 让 fork/clone/exec/exit 可以通过 `Arc` 生命周期表达共享和释放.
- 在任务切换时由 CPU 层统一激活页表.

非目标:

- 不实现 KPTI 或 per-process kernel mapping 随机化.
- 不在任务文档中展开 VMA,mmap,page fault 的完整策略.
- 不让内核线程拥有独立用户页表.

## 关键流程

### 切换任务

`current_cpu().switch_task` 设置当前任务.若目标任务不是内核线程, 会把 CPU 当前地址空间切到任务的 `memory_space` 并激活根页表.若目标任务是内核线程, 当前地址空间保持不变.

### exec

`execve` 创建或接收新的 `MemorySpace`, 替换当前任务地址空间, 关闭 `CLOEXEC` fd, 构造用户栈, 然后重建 `TrapFrame`.用户栈写入要求新地址空间已可访问.

### exit

进程 leader 退出时先切回全局内核页表, 再关闭 fd,分离 shared memory 并释放用户地址空间引用.线程退出只释放自己对共享地址空间的引用.

## 并发和生命周期约束

- 地址空间对象由 `Arc<SpinLock<MemorySpace>>` 共享, 修改映射需要持有对应锁.
- 释放当前正在使用的用户页表前必须先切换到全局内核页表.
- TLB 刷新和跨核 shootdown 属于内存管理与架构 IPI 协作边界, 任务层只表达地址空间所有权.
- 内核线程 `memory_space == None`, 调用 `current_memory_space()` 前必须确认当前 CPU 已有有效地址空间.

## 已知限制

- 当前模型优先性能和实现简单性, 内核映射在用户页表中可见, 没有 KPTI 隔离.
- 内核线程沿用 CPU 当前地址空间, 这要求内核线程不依赖用户映射语义.
- 更完整的多核 TLB shootdown 完成确认仍需继续完善.

## 源码索引

- `os/src/kernel/task/task_struct.rs`: `Task.memory_space`,exec 地址空间替换.
- `os/src/kernel/task/mod.rs`: `current_memory_space` 和进程退出资源清理.
- `os/src/kernel/cpu.rs`: `switch_task`,`switch_space` 和页表激活.
- `os/src/mm/mod.rs`: MM 初始化,全局内核地址空间记录和页表激活入口.
- `os/src/mm/memory_space/space/*`: `MemorySpace` 创建,clone,drop,mmap 和内核映射.
