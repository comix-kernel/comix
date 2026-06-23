//! 统一的系统调用分发逻辑
//!
//! 通过 `SyscallFrame` trait 实现架构无关的寄存器访问，
//! 使 `dispatch_syscall` 和 `impl_syscall!` 宏无需按架构复制。
//!
//! 每个架构只需在 trap_handler 中调用 `dispatch_syscall(trap_frame)`。

use crate::kernel::syscall::syscall_frame::SyscallFrame;
use crate::kernel::syscall::*;
use crate::uapi::errno::ENOSYS;

/// 分发系统调用（架构无关）。
pub fn dispatch_syscall(frame: &mut impl SyscallFrame) {
    crate::pr_debug!(
        "syscall: {} args: [{:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x}]",
        frame.syscall_id(),
        frame.arg0(),
        frame.arg1(),
        frame.arg2(),
        frame.arg3(),
        frame.arg4(),
        frame.arg5()
    );
    match frame.syscall_id() {
        // 文件系统/目录操作
        crate::kernel::syscall::numbers::SYS_GETCWD => sys_getcwd(frame),

        // Epoll & Duplication
        crate::kernel::syscall::numbers::SYS_DUP => sys_dup(frame),
        crate::kernel::syscall::numbers::SYS_DUP3 => sys_dup3(frame),
        crate::kernel::syscall::numbers::SYS_FCNTL => sys_fcntl(frame),
        crate::kernel::syscall::numbers::SYS_IOCTL => sys_ioctl(frame),

        // 文件/目录创建与链接
        crate::kernel::syscall::numbers::SYS_MKNODAT => sys_mknodat(frame),
        crate::kernel::syscall::numbers::SYS_MKDIRAT => sys_mkdirat(frame),
        crate::kernel::syscall::numbers::SYS_UNLINKAT => sys_unlinkat(frame),
        crate::kernel::syscall::numbers::SYS_SYMLINKAT => sys_symlinkat(frame),

        // 挂载/文件系统信息
        crate::kernel::syscall::numbers::SYS_MOUNT => sys_mount(frame),
        crate::kernel::syscall::numbers::SYS_UMOUNT2 => sys_umount2(frame),
        crate::kernel::syscall::numbers::SYS_STATFS => sys_statfs(frame),

        // 文件大小/权限/所有权
        crate::kernel::syscall::numbers::SYS_FACCESSAT => sys_faccessat(frame),
        crate::kernel::syscall::numbers::SYS_CHDIR => sys_chdir(frame),
        crate::kernel::syscall::numbers::SYS_FCHMODAT => sys_fchmodat(frame),
        crate::kernel::syscall::numbers::SYS_FCHOWNAT => sys_fchownat(frame),

        // 文件描述符操作
        crate::kernel::syscall::numbers::SYS_OPENAT => sys_openat(frame),
        crate::kernel::syscall::numbers::SYS_CLOSE => sys_close(frame),
        crate::kernel::syscall::numbers::SYS_PIPE2 => sys_pipe2(frame),
        crate::kernel::syscall::numbers::SYS_GETDENTS64 => sys_getdents64(frame),
        crate::kernel::syscall::numbers::SYS_LSEEK => sys_lseek(frame),
        crate::kernel::syscall::numbers::SYS_FTRUNCATE => sys_ftruncate(frame),

        // I/O 操作
        crate::kernel::syscall::numbers::SYS_READ => sys_read(frame),
        crate::kernel::syscall::numbers::SYS_WRITE => sys_write(frame),
        crate::kernel::syscall::numbers::SYS_READV => sys_readv(frame),
        crate::kernel::syscall::numbers::SYS_WRITEV => sys_writev(frame),
        crate::kernel::syscall::numbers::SYS_PREAD64 => sys_pread64(frame),
        crate::kernel::syscall::numbers::SYS_PWRITE64 => sys_pwrite64(frame),
        crate::kernel::syscall::numbers::SYS_PREADV => sys_preadv(frame),
        crate::kernel::syscall::numbers::SYS_PWRITEV => sys_pwritev(frame),
        crate::kernel::syscall::numbers::SYS_SENDFILE => sys_sendfile(frame),
        crate::kernel::syscall::numbers::SYS_PSELECT6 => sys_pselect6(frame),
        crate::kernel::syscall::numbers::SYS_PPOLL => sys_ppoll(frame),

        // 文件元数据与同步
        crate::kernel::syscall::numbers::SYS_READLINKAT => sys_readlinkat(frame),
        crate::kernel::syscall::numbers::SYS_FSTATAT => sys_newfstatat(frame),
        crate::kernel::syscall::numbers::SYS_FSTAT => sys_fstat(frame),
        crate::kernel::syscall::numbers::SYS_SYNC => sys_sync(frame),
        crate::kernel::syscall::numbers::SYS_FSYNC => sys_fsync(frame),
        crate::kernel::syscall::numbers::SYS_FDATASYNC => sys_fdatasync(frame),

        // 定时器
        crate::kernel::syscall::numbers::SYS_UTIMENSAT => sys_utimensat(frame),

        // 进程与控制
        crate::kernel::syscall::numbers::SYS_EXIT => sys_exit(frame),
        crate::kernel::syscall::numbers::SYS_EXIT_GROUP => sys_exit_group(frame),
        crate::kernel::syscall::numbers::SYS_SET_TID_ADDRESS => sys_set_tid_address(frame),

        // 同步/休眠
        crate::kernel::syscall::numbers::SYS_FUTEX => sys_futex(frame),
        crate::kernel::syscall::numbers::SYS_SET_ROBUST_LIST => sys_set_robust_list(frame),
        crate::kernel::syscall::numbers::SYS_GET_ROBUST_LIST => sys_get_robust_list(frame),
        crate::kernel::syscall::numbers::SYS_NANOSLEEP => sys_nanosleep(frame),
        crate::kernel::syscall::numbers::SYS_GETITIMER => sys_getitimmer(frame),
        crate::kernel::syscall::numbers::SYS_SETITIMER => sys_setitimmer(frame),

        // POSIX 定时器
        crate::kernel::syscall::numbers::SYS_CLOCK_SETTIME => sys_clock_settime(frame),
        crate::kernel::syscall::numbers::SYS_CLOCK_GETTIME => sys_clock_gettime(frame),
        crate::kernel::syscall::numbers::SYS_CLOCK_GETRES => sys_clock_getres(frame),
        crate::kernel::syscall::numbers::SYS_SYSLOG => sys_syslog(frame),

        // 调度
        crate::kernel::syscall::numbers::SYS_SCHED_YIELD => sys_sched_yield(frame),

        // 信号
        crate::kernel::syscall::numbers::SYS_KILL => sys_kill(frame),
        crate::kernel::syscall::numbers::SYS_TKILL => sys_tkill(frame),
        crate::kernel::syscall::numbers::SYS_TGKILL => sys_tgkill(frame),
        crate::kernel::syscall::numbers::SYS_SIGALTSTACK => sys_sigaltstack(frame),
        crate::kernel::syscall::numbers::SYS_RT_SIGSUSPEND => sys_rt_sigsuspend(frame),
        crate::kernel::syscall::numbers::SYS_RT_SIGACTION => sys_rt_sigaction(frame),
        crate::kernel::syscall::numbers::SYS_RT_SIGPROCMASK => sys_rt_sigprocmask(frame),
        crate::kernel::syscall::numbers::SYS_RT_SIGPENDING => sys_rt_sigpending(frame),
        crate::kernel::syscall::numbers::SYS_RT_SIGTIMEDWAIT => sys_rt_sigtimedwait(frame),
        crate::kernel::syscall::numbers::SYS_RT_SIGRETURN => sys_rt_sigreturn(frame),

        // 进程属性
        crate::kernel::syscall::numbers::SYS_REBOOT => sys_reboot(frame),
        crate::kernel::syscall::numbers::SYS_SETGID => sys_setgid(frame),
        crate::kernel::syscall::numbers::SYS_SETUID => sys_setuid(frame),
        crate::kernel::syscall::numbers::SYS_SETRESUID => sys_setresuid(frame),
        crate::kernel::syscall::numbers::SYS_GETRESUID => sys_getresuid(frame),
        crate::kernel::syscall::numbers::SYS_SETRESGID => sys_setresgid(frame),
        crate::kernel::syscall::numbers::SYS_GETRESGID => sys_getresgid(frame),
        crate::kernel::syscall::numbers::SYS_TIMES => sys_times(frame),
        crate::kernel::syscall::numbers::SYS_SETPGID => sys_setpgid(frame),
        crate::kernel::syscall::numbers::SYS_SETSID => sys_setsid(frame),

        // 系统信息
        crate::kernel::syscall::numbers::SYS_UNAME => sys_uname(frame),
        crate::kernel::syscall::numbers::SYS_SETHOSTNAME => sys_sethostname(frame),
        crate::kernel::syscall::numbers::SYS_GETRLIMIT => sys_getrlimit(frame),
        crate::kernel::syscall::numbers::SYS_SETRLIMIT => sys_setrlimit(frame),
        crate::kernel::syscall::numbers::SYS_UMASK => sys_umask(frame),
        crate::kernel::syscall::numbers::SYS_GETTIMEOFDAY => sys_gettimeofday(frame),
        crate::kernel::syscall::numbers::SYS_GETPID => sys_getpid(frame),
        crate::kernel::syscall::numbers::SYS_GETPPID => sys_getppid(frame),
        crate::kernel::syscall::numbers::SYS_GETPGID => sys_getpgid(frame),
        crate::kernel::syscall::numbers::SYS_GETUID => sys_getuid(frame),
        crate::kernel::syscall::numbers::SYS_GETEUID => sys_geteuid(frame),
        crate::kernel::syscall::numbers::SYS_GETGID => sys_getgid(frame),
        crate::kernel::syscall::numbers::SYS_GETEGID => sys_getegid(frame),
        crate::kernel::syscall::numbers::SYS_GETTID => sys_gettid(frame),
        crate::kernel::syscall::numbers::SYS_SYSINFO => sys_sysinfo(frame),

        // 网络
        crate::kernel::syscall::numbers::SYS_SOCKET => sys_socket(frame),
        crate::kernel::syscall::numbers::SYS_SOCKETPAIR => sys_socketpair(frame),
        crate::kernel::syscall::numbers::SYS_BIND => sys_bind(frame),
        crate::kernel::syscall::numbers::SYS_LISTEN => sys_listen(frame),
        crate::kernel::syscall::numbers::SYS_ACCEPT => sys_accept(frame),
        crate::kernel::syscall::numbers::SYS_CONNECT => sys_connect(frame),
        crate::kernel::syscall::numbers::SYS_GETSOCKNAME => sys_getsockname(frame),
        crate::kernel::syscall::numbers::SYS_GETPEERNAME => sys_getpeername(frame),
        crate::kernel::syscall::numbers::SYS_SENDTO => sys_sendto(frame),
        crate::kernel::syscall::numbers::SYS_RECVFROM => sys_recvfrom(frame),
        crate::kernel::syscall::numbers::SYS_SETSOCKOPT => sys_setsockopt(frame),
        crate::kernel::syscall::numbers::SYS_GETSOCKOPT => sys_getsockopt(frame),
        crate::kernel::syscall::numbers::SYS_SHUTDOWN => sys_shutdown(frame),

        // 进程创建/执行
        crate::kernel::syscall::numbers::SYS_CLONE => sys_clone(frame),
        crate::kernel::syscall::numbers::SYS_EXECVE => sys_execve(frame),

        // 网络 I/O (续)
        crate::kernel::syscall::numbers::SYS_ACCEPT4 => sys_accept4(frame),

        // 进程与控制 (续)
        crate::kernel::syscall::numbers::SYS_WAIT4 => sys_wait4(frame),
        crate::kernel::syscall::numbers::SYS_PRLIMIT64 => sys_prlimit(frame),

        // 内存管理
        crate::kernel::syscall::numbers::SYS_BRK => sys_brk(frame),
        crate::kernel::syscall::numbers::SYS_MUNMAP => sys_munmap(frame),
        crate::kernel::syscall::numbers::SYS_MMAP => sys_mmap(frame),
        crate::kernel::syscall::numbers::SYS_MPROTECT => sys_mprotect(frame),

        // 文件系统同步 (续)
        crate::kernel::syscall::numbers::SYS_SYNCFS => sys_syncfs(frame),

        // 调度 (续)
        crate::kernel::syscall::numbers::SYS_RENAMEAT2 => sys_renameat2(frame),

        // 随机数与内存文件
        crate::kernel::syscall::numbers::SYS_GETRANDOM => sys_getrandom(frame),

        // 扩展文件元数据
        crate::kernel::syscall::numbers::SYS_STATX => sys_statx(frame),

        // 获取网络接口地址列表
        crate::kernel::syscall::numbers::SYS_GETIFADDRS => sys_getifaddrs(frame),

        _ => {
            frame.set_ret((-ENOSYS) as usize);
            crate::pr_debug!("Unknown syscall: {}", frame.syscall_id());
        }
    }
    crate::pr_debug!("syscall exit, return: {}", frame.arg0() as isize);
}

/// 宏：实现系统调用包装器（架构无关版本）。
///
/// 生成的函数签名为 `(frame: &mut impl SyscallFrame)`，
/// 从 frame 提取参数、调用内核实现、写回返回值。
#[macro_export]
macro_rules! impl_syscall {
    // noreturn, 6 args
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty, $t5:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(
            frame: &mut impl $crate::kernel::syscall::syscall_frame::SyscallFrame,
        ) -> ! {
            let a0 = frame.arg0() as $t0;
            let a1 = frame.arg1() as $t1;
            let a2 = frame.arg2() as $t2;
            let a3 = frame.arg3() as $t3;
            let a4 = frame.arg4() as $t4;
            let a5 = frame.arg5() as $t5;
            $kernel(a0, a1, a2, a3, a4, a5)
        }
    };

    // noreturn, 0..5 args
    ($sys_name:ident, $kernel:path, noreturn, ()) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(
            _frame: &mut impl $crate::kernel::syscall::syscall_frame::SyscallFrame,
        ) -> ! {
            $kernel()
        }
    };
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(
            frame: &mut impl $crate::kernel::syscall::syscall_frame::SyscallFrame,
        ) -> ! {
            let a0 = frame.arg0() as $t0;
            $kernel(a0)
        }
    };
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(
            frame: &mut impl $crate::kernel::syscall::syscall_frame::SyscallFrame,
        ) -> ! {
            let a0 = frame.arg0() as $t0;
            let a1 = frame.arg1() as $t1;
            $kernel(a0, a1)
        }
    };
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty, $t2:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(
            frame: &mut impl $crate::kernel::syscall::syscall_frame::SyscallFrame,
        ) -> ! {
            let a0 = frame.arg0() as $t0;
            let a1 = frame.arg1() as $t1;
            let a2 = frame.arg2() as $t2;
            $kernel(a0, a1, a2)
        }
    };
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty, $t2:ty, $t3:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(
            frame: &mut impl $crate::kernel::syscall::syscall_frame::SyscallFrame,
        ) -> ! {
            let a0 = frame.arg0() as $t0;
            let a1 = frame.arg1() as $t1;
            let a2 = frame.arg2() as $t2;
            let a3 = frame.arg3() as $t3;
            $kernel(a0, a1, a2, a3)
        }
    };
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(
            frame: &mut impl $crate::kernel::syscall::syscall_frame::SyscallFrame,
        ) -> ! {
            let a0 = frame.arg0() as $t0;
            let a1 = frame.arg1() as $t1;
            let a2 = frame.arg2() as $t2;
            let a3 = frame.arg3() as $t3;
            let a4 = frame.arg4() as $t4;
            $kernel(a0, a1, a2, a3, a4)
        }
    };

    // returning, 6 args
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty, $t5:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut impl $crate::kernel::syscall::syscall_frame::SyscallFrame) {
            let a0 = frame.arg0() as $t0;
            let a1 = frame.arg1() as $t1;
            let a2 = frame.arg2() as $t2;
            let a3 = frame.arg3() as $t3;
            let a4 = frame.arg4() as $t4;
            let a5 = frame.arg5() as $t5;
            let ret = $kernel(a0, a1, a2, a3, a4, a5);
            frame.set_ret(ret as isize as usize);
        }
    };

    // returning, 0..5 args
    ($sys_name:ident, $kernel:path, ()) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut impl $crate::kernel::syscall::syscall_frame::SyscallFrame) {
            let ret = $kernel();
            frame.set_ret(ret as isize as usize);
        }
    };
    ($sys_name:ident, $kernel:path, ($t0:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut impl $crate::kernel::syscall::syscall_frame::SyscallFrame) {
            let a0 = frame.arg0() as $t0;
            let ret = $kernel(a0);
            frame.set_ret(ret as isize as usize);
        }
    };
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut impl $crate::kernel::syscall::syscall_frame::SyscallFrame) {
            let a0 = frame.arg0() as $t0;
            let a1 = frame.arg1() as $t1;
            let ret = $kernel(a0, a1);
            frame.set_ret(ret as isize as usize);
        }
    };
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut impl $crate::kernel::syscall::syscall_frame::SyscallFrame) {
            let a0 = frame.arg0() as $t0;
            let a1 = frame.arg1() as $t1;
            let a2 = frame.arg2() as $t2;
            let ret = $kernel(a0, a1, a2);
            frame.set_ret(ret as isize as usize);
        }
    };
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty, $t3:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut impl $crate::kernel::syscall::syscall_frame::SyscallFrame) {
            let a0 = frame.arg0() as $t0;
            let a1 = frame.arg1() as $t1;
            let a2 = frame.arg2() as $t2;
            let a3 = frame.arg3() as $t3;
            let ret = $kernel(a0, a1, a2, a3);
            frame.set_ret(ret as isize as usize);
        }
    };
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut impl $crate::kernel::syscall::syscall_frame::SyscallFrame) {
            let a0 = frame.arg0() as $t0;
            let a1 = frame.arg1() as $t1;
            let a2 = frame.arg2() as $t2;
            let a3 = frame.arg3() as $t3;
            let a4 = frame.arg4() as $t4;
            let ret = $kernel(a0, a1, a2, a3, a4);
            frame.set_ret(ret as isize as usize);
        }
    };
}
