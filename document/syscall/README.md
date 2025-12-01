# Syscall 概览

本文档收录了两百个左右Linux x86_64 的主干 syscall 接口，用于在实现系统调用时速查。

说明：作用按领域分组；参数一般遵循 (int fd, const char *buf, size_t len)、(const struct TimeSpec *req, struct TimeSpec *rem)、(int pid, int sig)、(void *addr, size_t len, int prot, int flags, int fd, off_t off) 等常见模式。下面按类别简述关键功能与典型参数。未逐条展开，保持速览。

1. 基础文件与 IO
- read/write/pread/pwrite/readv/writev: (fd, buf/vec, count, offset)
- open/openat/close/creat: (path/dirfd, flags, mode)
- lseek: (fd, offset, whence)
- fstat/stat/lstat/newfstatat: (path/fd, statbuf)
- fsync/fdatasync/fallocate/truncate/ftruncate: (fd, len)
- getdents/getdents64: (fd, dirent_buf, size)
- access/faccessat/faccessat2: (path/dirfd, mode, flags)
- chmod/fchmod/fchmodat/fchmodat2: (path/fd, mode)
- chown/fchown/lchown/fchownat: (path/fd, uid, gid)
- link/symlink/unlink/[at]、readlink[at]: (old, new)/(path, buf, size)
- rename/renameat/renameat2: (old,new[,flags])
- mkdir/mkdirat/rmdir/mknod/mknodat: (path[,mode,type])
- utime/utimensat/utimes: (path, times)
- statx: (dirfd, path, flags, mask, statxbuf)

2. 进程与线程
- fork/vfork/clone/clone3: (flags, stack, parent_tid, child_tid, tls)
- execve/execveat: (path, argv, envp[,flags])
- exit/exit_group: (code)
- wait4/waitid: (pid, status, options, rusage)
- getpid/getppid/gettid: 无参或返回当前 ID
- setuid/setgid/setreuid/...： (uid/gid 组合)
- setpgid/getsid/setsid/getpgid: (pid, pgid)
- prctl/personality: (option, arg1..)
- sched_yield: 无参；sched_set/getparam/scheduler/affinity/attr： (pid, param/attr/mask)
- set_tid_address: (tidptr) 线程退出清理
- restart_syscall: 内核透明重启阻塞的调用

3. 内存管理
- mmap/mmap2/mremap/munmap: (addr, len, prot, flags, fd, off)
- mprotect: (addr, len, prot)
- brk: (addr)
- madvise/mincore: (addr, len, advice)/查询驻留
- mlock/mlockall/munlock/munlockall: (addr, len)
- memfd_create: (name, flags)
- mlock2/pkey_alloc/pkey_free/pkey_mprotect: 内存保护键
- process_madvise: (pidfd, iov, advice)
- map_shadow_stack/mseal: 安全栈/内存封印
- set_mempolicy/get_mempolicy/mbind: NUMA 策略
- migrate_pages/move_pages: (pid, list)

4. 信号与计时
- rt_sigaction/rt_sigprocmask/rt_sigpending/rt_sigsuspend/rt_sigqueueinfo/rt_tgsigqueueinfo: (signum, act, mask, size)
- kill/tgkill/tkill: (pid[/tgid], tid, sig)
- rt_sigreturn: 用户态返回栈恢复
- timer_create/timer_settime/timer_gettime/timerfd_*: POSIX/FD 定时器
- nanosleep/clock_nanosleep: (req, rem[, clock, flags])
- getitimer/setitimer/alarm: 定时器
- gettimeofday/clock_gettime/clock_settime/clock_getres/adjtimex: 时间与校时
- times: (tmsbuf)
- getrusage: (who, rusage)
- sched_rr_get_interval: (pid, TimeSpec)

5. IPC
- pipe/pipe2: (fds[2])
- socket/socketpair/bind/listen/accept/accept4/connect: 套接字基本
- sendto/recvfrom/sendmsg/recvmsg/sendmmsg/recvmmsg/shutdown: 数据收发 (fd, buf/msg, len, flags, addr)
- setsockopt/getsockopt: (fd, level, optname, optval)
- epoll_create/epoll_wait/epoll_ctl/epoll_pwait/epoll_create1/poll/ppoll/select/pselect: 事件复用
- eventfd/eventfd2/signalfd/signalfd4/timerfd_create/inotify_*： 各类 FD 通知
- futex/futex_waitv/futex_wake/futex_wait/futex_requeue: (uaddr, op, val, timeout, uaddr2, val3)
- msgget/msgsnd/msgrcv/msgctl: System V 消息队列
- semget/semop/semctl/semtimedop: System V 信号量
- shmget/shmat/shmdt/shmctl: 共享内存
- memfd_secret: 私密匿名内存
- mq_open/mq_timedsend/mq_timedreceive/mq_getsetattr/mq_unlink/mq_notify: POSIX 消息队列
- add_key/request_key/keyctl: Key 管理
- process_vm_readv/writev: 跨进程内存访问
- pidfd_open/pidfd_getfd/pidfd_send_signal: 稳定 PID 引用

6. 文件系统与挂载
- mount/umount2/move_mount/open_tree/openat2/fsopen/fsconfig/fsmount/fspick/quotactl/quotactl_fd: 挂载与文件系统管理
- statfs/fstatfs: 文件系统状态
- sync/syncfs: 刷写
- renameat2: 原子重命名
- open_tree_attr/file_getattr/file_setattr: 树与属性

7. 安全与权限
- capget/capset: 进程能力
- seccomp: 沙箱过滤
- bpf: 加载 BPF 程序
- setns: 进入命名空间
- setxattr/getxattr/listxattr/removexattr 及 *at 变体：扩展属性
- chroot: 改根
- ptrace: (request, pid, addr, data)
- landlock_*： Landlock 安全沙箱
- lsm_*: LSM 自省接口
- process_mrelease: 释放僵尸进程资源

8. 性能与异步 IO
- readahead/fadvise64: 预读与访问提示
- io_setup/io_submit/io_getevents/io_destroy/io_cancel/io_pgetevents： AIO
- splice/tee/vmsplice: 零拷贝管道操作
- copy_file_range: 零拷贝文件段传输
- perf_event_open: 性能监控

9. 资源与限制
- getrlimit/setrlimit/prlimit64: 资源限制
- getpriority/setpriority/nice: 调度优先级
- rlimit 相关参数： (which, rlimit struct)

10. 进程凭据与组
- getuid/geteuid/getgid/getegid/getgroups/setgroups/setresuid/getresuid/setresgid/getresgid/setfsuid/setfsgid： 用户/组 ID 操作

11. 其它杂项
- uname: (utsname)
- sysinfo: (info)
- reboot: (magic, magic2, cmd, arg)
- adjtimex: (timex)
- kexec_load/kexec_file_load: 内核热重启
- rseq: (rseq_area, len, flags, sig)
- cachestat: 文件缓存统计
- statmount/listmount: 挂载枚举
- map_shadow_stack: 安全栈映射

参数模式速记
- 路径相关：dirfd + path + flags + mode
- IO 向量：iovec 数组 + count（readv/writev/preadv/pwritev）
- 时间：TimeSpec/timeval + 可选剩余/精度结构
- 结构读写：用户指针传入，内核填充（stat, rusage, utsname）
- 标志位：按 OR 组合（O_*、MAP_*、EPOLL*、SOCK_*）

获取详细参数/返回值
- man 2 <name>
- 内核源码：include/uapi/asm-generic/ 或 arch/x86/include/asm/
- 返回值约定：负 errno 放入 -Exxx，用户态转换为 errno。

如需具体某组 syscall 详细参数/错误码，再指定名称列表。