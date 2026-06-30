# Syscall 子系统设计

Syscall 层是用户态 ABI 到内核子系统的边界。当前文档只描述 Comix 当前实现的分发, frame 抽象, 用户指针和子系统入口, 不维护庞大的 Linux syscall 清单。

## 当前状态

- `dispatch_syscall()` 按 syscall number 分发到 `sys_*` 包装函数。
- `SyscallFrame` trait 抽象寄存器读取和返回值写回, 让分发逻辑不绑定具体架构。
- `impl_syscall!` 宏负责从 frame 提取最多 6 个参数, 转换为 Rust 函数签名, 并把返回值写回 frame。
- 真正实现按领域拆到 `fs`, `io`, `task`, `mm`, `signal`, `ipc`, `network`, `sys`, `cred` 等模块。
- 未识别 syscall 返回 `-ENOSYS`。

## 目标

- 让架构相关 trap frame 和通用 syscall 实现解耦。
- 让 syscall 层承担 ABI 边界职责: 参数解码, 用户指针复制, fd 查找, errno 映射。
- 让具体业务逻辑留在对应内核子系统, syscall 模块只做薄适配。

## 非目标

- 不在文档维护完整 syscall 号码表。号码以 `numbers.rs` 为准。
- 不在文档列出每个 syscall 的参数和错误分支。细节以源码和 rustdoc 为准。
- 不把 syscall 层当作跨子系统共享状态的宿主。

## Dispatch 边界

系统调用入口在架构 trap handler 中取得当前 trap frame 后调用 `dispatch_syscall(frame)`。dispatch 只做三件事:

1. 读取 syscall id 和参数用于调试日志。
2. 根据 `numbers.rs` 中的常量选择对应 `sys_*` wrapper。
3. 对未知号码写回 `-ENOSYS`。

dispatch 不直接读取用户内存, 不操作 fd table, 不进入 VFS 或协议栈内部状态。

## SyscallFrame 抽象

`SyscallFrame` 是架构无关寄存器接口:

- `syscall_id()` 读取 syscall number。
- `arg0()` 到 `arg5()` 读取 ABI 参数寄存器。
- `set_ret()` 写回返回值。

RISC-V, LoongArch 等架构只需要把自己的 trap frame 适配到这个 trait, 通用 syscall 模块就可以复用同一套分发和 wrapper。

## impl_syscall wrapper

`impl_syscall!` 生成 `sys_*` 函数。生成代码负责:

- 从 frame 取原始 `usize` 参数。
- 按声明转换为指针, 整数或 ABI 结构指针。
- 调用实际内核实现函数。
- 对普通返回 syscall 写回 `isize` 返回值。
- 对 `noreturn` syscall 直接转入不返回路径, 如 `exit_group` 或 `rt_sigreturn`。

这层不做深度验证。验证必须在具体实现函数中完成, 因为只有实现函数知道参数含义和所需锁顺序。

## Errno 和返回值

当前约定是内核实现返回 `isize` 或 `c_int`, 成功返回非负结果, 失败返回负 errno。常见来源:

- VFS 错误通过 `FsError::to_errno()` 转换。
- 网络错误通过 `NetworkError::to_errno()` 或具体 syscall 映射。
- UAPI errno 常量来自 `uapi::errno`。
- `brk` 这类 Linux 兼容接口按自身 ABI 返回当前 brk, 而不是负 errno。

新增 syscall 时应优先使用已有错误类型转换, 避免在多个模块散落私有错误码。

## 用户指针边界

用户指针只能在 syscall 边界或明确的用户缓冲工具中解引用。当前主要路径:

- 字符串路径: `util::get_path_safe()` 和 `copy_str_from_user()`。
- argv/envp: `get_args_safe()`。
- I/O 缓冲: `kernel/syscall/io.rs` 中先复制到内核 `Vec`, 再调用 `File`。
- sockaddr: `kernel/syscall/network/*` 负责解析和写回。
- shm/stat/signal/syslog: 各自 syscall 模块用 `read_from_user`, `write_to_user` 或架构 copy helper。

设计要求:

- 不把用户指针保存到内核对象中。
- 持有协议栈, VFS, MemorySpace 等关键锁时避免长时间复制用户缓冲区。
- 复制失败返回 `-EFAULT` 或对应子系统错误。

## 子系统入口

- FS: `fs/**`, `fcntl.rs`, `ioctl.rs` 处理路径, fd, mount, stat, rename 等。
- IO: `io.rs` 处理 read/write/readv/writev/poll/ppoll/pselect 等通用 fd I/O。
- Task: `task/**` 处理 clone, exec, exit, wait, futex, sched, time。
- MM: `mm.rs` 处理 brk, mmap, munmap, mprotect。
- Signal: `signal.rs` 处理 rt_sigaction, rt_sigprocmask, sigtimedwait, sigreturn 等。
- IPC: `ipc.rs` 处理 pipe2, dup, SysV shm。
- Network: `network/**` 处理 socket, bind, connect, accept, send/recv, sockopt, ifaddrs。
- System/log: `sys.rs` 处理 uname, sysinfo, syslog, reboot 等系统级接口。
- Credentials: `cred.rs` 处理 uid/gid 相关接口。

## 并发和生命周期约束

- syscall wrapper 不持有锁跨越实际实现调用。
- fd 操作必须考虑 close/dup/fork/exec 的共享表语义。
- exec 会先执行 close-on-exec, detach SysV shm, 再切换地址空间和 trap frame。
- exit_group 走进程级资源清理, 包括 fd, socket fd mapping, shm attachment 和地址空间。
- poll/select waiters 和网络 poll 通过 `io.rs` 与 `net::socket` 协作, 避免在硬中断中推进 smoltcp。

## 已知限制

- syscall 支持范围由 `numbers.rs` 和 `dispatch.rs` 的匹配分支决定, 并不等价于完整 Linux ABI。
- 部分 syscall 为兼容测试提供最小语义, 不代表完整内核实现。
- `SA_RESTART` 等高级 syscall restart 语义尚不完整, 阻塞调用可能返回 `EINTR`。

## 源码索引

- `os/src/kernel/syscall/mod.rs`: 模块组织和 `impl_syscall!` 注册。
- `os/src/kernel/syscall/dispatch.rs`: 架构无关分发和 wrapper 宏。
- `os/src/kernel/syscall/syscall_frame.rs`: `SyscallFrame` trait。
- `os/src/kernel/syscall/numbers.rs`: 当前处理的 syscall number。
- `os/src/kernel/syscall/util.rs`: 路径, argv/envp, syslog 参数辅助。
- `os/src/kernel/syscall/fs/`: 文件系统 syscall。
- `os/src/kernel/syscall/io.rs`: 通用 fd I/O 和 poll。
- `os/src/kernel/syscall/network/`: socket syscall。
- `os/src/kernel/syscall/task/`: 任务和进程 syscall。
- `os/src/kernel/syscall/mm.rs`: 内存 syscall。
- `os/src/kernel/syscall/signal.rs`: 信号 syscall。
- `os/src/kernel/syscall/ipc.rs`: pipe 和 SysV shm。
