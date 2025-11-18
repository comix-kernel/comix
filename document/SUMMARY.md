# 目录

[介绍](README.md)

# 内存管理

- [内存管理概述](mm/README.md)
  - [架构设计](mm/architecture.md)
  - [地址抽象](mm/address.md)
  - [物理帧分配器](mm/frame_allocator.md)
  - [页表管理](mm/page_table.md)
  - [内存空间](mm/memory_space.md)
  - [全局分配器](mm/global_allocator.md)
  - [API 参考](mm/api_reference.md)

# 日志系统

- [日志系统概述](log/README.md)
  - [架构设计](log/architecture.md)
  - [日志级别](log/level.md)
  - [缓冲区和条目](log/buffer_and_entry.md)
  - [使用方法](log/usage.md)
  - [API 参考](log/api_reference.md)

# 同步原语

- [同步机制概述](sync/README.md)
  - [自旋锁](sync/spin_lock.md)
  - [睡眠锁](sync/sleep_lock.md)
  - [中断保护](sync/intr_guard.md)
  - [SMP 与中断](sync/smp_interrupts.md)
  - [死锁检测](sync/deadlock.md)

# 内核子系统

## 任务管理

- [任务管理概述](kernel/task/README.md)
  - [任务结构](kernel/task/task.md)
  - [调度器](kernel/task/scheduler.md)
  - [上下文切换](kernel/task/context.md)
  - [内存空间](kernel/task/memory_space.md)
  - [等待队列](kernel/task/wait_queue.md)

## 中断与陷阱

- [中断处理](kernel/trap/trap.md)
  - [上下文保存](kernel/trap/context.md)
  - [特权级切换](kernel/trap/switch.md)

# 进程间通信
- [进程间通信概述](ipc/README.md)
  - [管道](ipc/pipe.md)
  - [消息](ipc/message.md)
  - [共享内存](ipc/shared_memory.md)
  - [信号](ipc/signal.md)
  - [信号生命周期](ipc/signal_lifecycle.md)

# 系统调用

- [系统调用速查](syscall/README.md)

# 架构相关

## RISC-V

- [RISC-V寄存器](arch/riscv/riscv_register.md)
- [用户栈布局](arch/riscv/stack_layout.md)


---

- [脚本工具](scripts/README.md)
  - [SimpleFS 镜像打包](scripts/make_init_simple_fs.md)
  - [文档链接转换](scripts/rewrite_links.md)
  - [代码质量检查](scripts/style-check.md)

---

[API 文档](api.md)
