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
        crate::arch::syscall::SYS_GETCWD => sys_getcwd(frame),

        // Epoll & Duplication
        crate::arch::syscall::SYS_DUP => sys_dup(frame),
        crate::arch::syscall::SYS_DUP3 => sys_dup3(frame),
        crate::arch::syscall::SYS_FCNTL => sys_fcntl(frame),
        crate::arch::syscall::SYS_IOCTL => sys_ioctl(frame),

        // 文件/目录创建与链接
        crate::arch::syscall::SYS_MKNODAT => sys_mknodat(frame),
        crate::arch::syscall::SYS_MKDIRAT => sys_mkdirat(frame),
        crate::arch::syscall::SYS_UNLINKAT => sys_unlinkat(frame),
        crate::arch::syscall::SYS_SYMLINKAT => sys_symlinkat(frame),

        // 挂载/文件系统信息
        crate::arch::syscall::SYS_MOUNT => sys_mount(frame),
        crate::arch::syscall::SYS_UMOUNT2 => sys_umount2(frame),
        crate::arch::syscall::SYS_STATFS => sys_statfs(frame),

        // 文件大小/权限/所有权
        crate::arch::syscall::SYS_FACCESSAT => sys_faccessat(frame),
        crate::arch::syscall::SYS_CHDIR => sys_chdir(frame),
        crate::arch::syscall::SYS_FCHMODAT => sys_fchmodat(frame),
        crate::arch::syscall::SYS_FCHOWNAT => sys_fchownat(frame),

        // 文件描述符操作
        crate::arch::syscall::SYS_OPENAT => sys_openat(frame),
        crate::arch::syscall::SYS_CLOSE => sys_close(frame),
        crate::arch::syscall::SYS_PIPE2 => sys_pipe2(frame),
        crate::arch::syscall::SYS_GETDENTS64 => sys_getdents64(frame),
        crate::arch::syscall::SYS_LSEEK => sys_lseek(frame),
        crate::arch::syscall::SYS_FTRUNCATE => sys_ftruncate(frame),

        // I/O 操作
        crate::arch::syscall::SYS_READ => sys_read(frame),
        crate::arch::syscall::SYS_WRITE => sys_write(frame),
        crate::arch::syscall::SYS_READV => sys_readv(frame),
        crate::arch::syscall::SYS_WRITEV => sys_writev(frame),
        crate::arch::syscall::SYS_PREAD64 => sys_pread64(frame),
        crate::arch::syscall::SYS_PWRITE64 => sys_pwrite64(frame),
        crate::arch::syscall::SYS_PREADV => sys_preadv(frame),
        crate::arch::syscall::SYS_PWRITEV => sys_pwritev(frame),
        crate::arch::syscall::SYS_SENDFILE => sys_sendfile(frame),
        crate::arch::syscall::SYS_PSELECT6 => sys_pselect6(frame),
        crate::arch::syscall::SYS_PPOLL => sys_ppoll(frame),

        // 文件元数据与同步
        crate::arch::syscall::SYS_READLINKAT => sys_readlinkat(frame),
        crate::arch::syscall::SYS_FSTATAT => sys_newfstatat(frame),
        crate::arch::syscall::SYS_FSTAT => sys_fstat(frame),
        crate::arch::syscall::SYS_SYNC => sys_sync(frame),
        crate::arch::syscall::SYS_FSYNC => sys_fsync(frame),
        crate::arch::syscall::SYS_FDATASYNC => sys_fdatasync(frame),

        // 定时器
        crate::arch::syscall::SYS_UTIMENSAT => sys_utimensat(frame),

        // 进程与控制
        crate::arch::syscall::SYS_EXIT => sys_exit(frame),
        crate::arch::syscall::SYS_EXIT_GROUP => sys_exit_group(frame),
        crate::arch::syscall::SYS_SET_TID_ADDRESS => sys_set_tid_address(frame),

        // 同步/休眠
        crate::arch::syscall::SYS_FUTEX => sys_futex(frame),
        crate::arch::syscall::SYS_SET_ROBUST_LIST => sys_set_robust_list(frame),
        crate::arch::syscall::SYS_GET_ROBUST_LIST => sys_get_robust_list(frame),
        crate::arch::syscall::SYS_NANOSLEEP => sys_nanosleep(frame),
        crate::arch::syscall::SYS_GETITIMER => sys_getitimmer(frame),
        crate::arch::syscall::SYS_SETITIMER => sys_setitimmer(frame),

        // POSIX 定时器
        crate::arch::syscall::SYS_CLOCK_SETTIME => sys_clock_settime(frame),
        crate::arch::syscall::SYS_CLOCK_GETTIME => sys_clock_gettime(frame),
        crate::arch::syscall::SYS_CLOCK_GETRES => sys_clock_getres(frame),
        crate::arch::syscall::SYS_SYSLOG => sys_syslog(frame),

        // 信号
        crate::arch::syscall::SYS_KILL => sys_kill(frame),
        crate::arch::syscall::SYS_TKILL => sys_tkill(frame),
        crate::arch::syscall::SYS_TGKILL => sys_tgkill(frame),
        crate::arch::syscall::SYS_SIGALTSTACK => sys_sigaltstack(frame),
        crate::arch::syscall::SYS_RT_SIGSUSPEND => sys_rt_sigsuspend(frame),
        crate::arch::syscall::SYS_RT_SIGACTION => sys_rt_sigaction(frame),
        crate::arch::syscall::SYS_RT_SIGPROCMASK => sys_rt_sigprocmask(frame),
        crate::arch::syscall::SYS_RT_SIGPENDING => sys_rt_sigpending(frame),
        crate::arch::syscall::SYS_RT_SIGTIMEDWAIT => sys_rt_sigtimedwait(frame),
        crate::arch::syscall::SYS_RT_SIGRETURN => sys_rt_sigreturn(frame),

        // 进程属性
        crate::arch::syscall::SYS_REBOOT => sys_reboot(frame),
        crate::arch::syscall::SYS_SETGID => sys_setgid(frame),
        crate::arch::syscall::SYS_SETUID => sys_setuid(frame),
        crate::arch::syscall::SYS_SETRESUID => sys_setresuid(frame),
        crate::arch::syscall::SYS_GETRESUID => sys_getresuid(frame),
        crate::arch::syscall::SYS_SETRESGID => sys_setresgid(frame),
        crate::arch::syscall::SYS_GETRESGID => sys_getresgid(frame),
        crate::arch::syscall::SYS_SETPGID => sys_setpgid(frame),
        crate::arch::syscall::SYS_SETSID => sys_setsid(frame),

        // 系统信息
        crate::arch::syscall::SYS_UNAME => sys_uname(frame),
        crate::arch::syscall::SYS_SETHOSTNAME => sys_sethostname(frame),
        crate::arch::syscall::SYS_GETRLIMIT => sys_getrlimit(frame),
        crate::arch::syscall::SYS_SETRLIMIT => sys_setrlimit(frame),
        crate::arch::syscall::SYS_UMASK => sys_umask(frame),
        crate::arch::syscall::SYS_GETPID => sys_getpid(frame),
        crate::arch::syscall::SYS_GETPPID => sys_getppid(frame),
        crate::arch::syscall::SYS_GETPGID => sys_getpgid(frame),
        crate::arch::syscall::SYS_GETUID => sys_getuid(frame),
        crate::arch::syscall::SYS_GETEUID => sys_geteuid(frame),
        crate::arch::syscall::SYS_GETGID => sys_getgid(frame),
        crate::arch::syscall::SYS_GETEGID => sys_getegid(frame),
        crate::arch::syscall::SYS_GETTID => sys_gettid(frame),
        crate::arch::syscall::SYS_SYSINFO => sys_sysinfo(frame),

        // 网络
        crate::arch::syscall::SYS_SOCKET => sys_socket(frame),
        crate::arch::syscall::SYS_BIND => sys_bind(frame),
        crate::arch::syscall::SYS_LISTEN => sys_listen(frame),
        crate::arch::syscall::SYS_ACCEPT => sys_accept(frame),
        crate::arch::syscall::SYS_CONNECT => sys_connect(frame),
        crate::arch::syscall::SYS_GETSOCKNAME => sys_getsockname(frame),
        crate::arch::syscall::SYS_GETPEERNAME => sys_getpeername(frame),
        crate::arch::syscall::SYS_SENDTO => sys_sendto(frame),
        crate::arch::syscall::SYS_RECVFROM => sys_recvfrom(frame),
        crate::arch::syscall::SYS_SETSOCKOPT => sys_setsockopt(frame),
        crate::arch::syscall::SYS_GETSOCKOPT => sys_getsockopt(frame),
        crate::arch::syscall::SYS_SHUTDOWN => sys_shutdown(frame),

        // 进程创建/执行
        crate::arch::syscall::SYS_CLONE => sys_clone(frame),
        crate::arch::syscall::SYS_EXECVE => sys_execve(frame),

        // 网络 I/O (续)
        crate::arch::syscall::SYS_ACCEPT4 => sys_accept4(frame),

        // 进程与控制 (续)
        crate::arch::syscall::SYS_WAIT4 => sys_wait4(frame),
        crate::arch::syscall::SYS_PRLIMIT64 => sys_prlimit(frame),

        // 内存管理
        crate::arch::syscall::SYS_BRK => sys_brk(frame),
        crate::arch::syscall::SYS_MUNMAP => sys_munmap(frame),
        crate::arch::syscall::SYS_MMAP => sys_mmap(frame),
        crate::arch::syscall::SYS_MPROTECT => sys_mprotect(frame),

        // 文件系统同步 (续)
        crate::arch::syscall::SYS_SYNCFS => sys_syncfs(frame),

        // 调度 (续)
        crate::arch::syscall::SYS_RENAMEAT2 => sys_renameat2(frame),

        // 随机数与内存文件
        crate::arch::syscall::SYS_GETRANDOM => sys_getrandom(frame),

        // 扩展文件元数据
        crate::arch::syscall::SYS_STATX => sys_statx(frame),

        // 获取网络接口地址列表
        crate::arch::syscall::SYS_GETIFADDRS => sys_getifaddrs(frame),

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
