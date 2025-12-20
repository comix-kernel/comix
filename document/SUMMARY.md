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

# 虚拟文件系统

- [VFS 概述](vfs/README.md)
  - [整体架构](vfs/architecture.md)
  - [Inode 与 Dentry](vfs/inode_and_dentry.md)
  - [File 与 FDTable](vfs/file_and_fdtable.md)
  - [路径解析与挂载](vfs/path_and_mount.md)
  - [FileSystem 与错误处理](vfs/filesystem_and_errors.md)
  - [文件锁与设备管理](vfs/filelock_and_devices.md)
  - [使用指南](vfs/usage.md)

# 文件系统实现

- [FS 模块概述](fs/README.md)
  - [Tmpfs - 临时文件系统](fs/tmpfs.md)
  - [ProcFS - 进程信息](fs/procfs.md)
  - [SysFS - 系统设备](fs/sysfs.md)
  - [Ext4 - Linux文件系统](fs/ext4.md)
  - [SimpleFS - 测试文件系统](fs/simple_fs.md)

# 设备与驱动

- [设备与驱动概览](devices/README.md)

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
