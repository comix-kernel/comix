//! RISC-V 架构的系统调用分发模块
use crate::kernel::syscall::*;
use crate::uapi::errno::ENOSYS;

mod syscall_number;

/// 分发系统调用
/// 按照系统调用号顺序排列，参考 syscall_number.rs 中的分类
pub fn dispatch_syscall(frame: &mut super::trap::TrapFrame) {
    crate::pr_debug!(
        "syscall: {} args: [{:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x}]",
        frame.x17_a7,
        frame.x10_a0,
        frame.x11_a1,
        frame.x12_a2,
        frame.x13_a3,
        frame.x14_a4,
        frame.x15_a5
    );
    match frame.x17_a7 {
        // 文件系统/目录操作 (Filesystem/Directory Operations)
        syscall_number::SYS_GETCWD => sys_getcwd(frame),

        // Epoll & Duplication
        syscall_number::SYS_DUP => sys_dup(frame),
        syscall_number::SYS_DUP3 => sys_dup3(frame),
        syscall_number::SYS_FCNTL => sys_fcntl(frame),
        syscall_number::SYS_IOCTL => sys_ioctl(frame),

        // 文件/目录创建与链接 (File/Directory Creation and Linking)
        syscall_number::SYS_MKNODAT => sys_mknodat(frame),
        syscall_number::SYS_MKDIRAT => sys_mkdirat(frame),
        syscall_number::SYS_UNLINKAT => sys_unlinkat(frame),
        syscall_number::SYS_SYMLINKAT => sys_symlinkat(frame),

        // 挂载/文件系统信息 (Mount/Filesystem Info)
        syscall_number::SYS_MOUNT => sys_mount(frame),
        syscall_number::SYS_UMOUNT2 => sys_umount2(frame),
        syscall_number::SYS_STATFS => sys_statfs(frame),

        // 文件大小/权限/所有权 (File Size/Permissions/Ownership)
        syscall_number::SYS_FACCESSAT => sys_faccessat(frame),
        syscall_number::SYS_CHDIR => sys_chdir(frame),
        syscall_number::SYS_FCHMODAT => sys_fchmodat(frame),
        syscall_number::SYS_FCHOWNAT => sys_fchownat(frame),

        // 文件描述符操作 (File Descriptor Operations)
        syscall_number::SYS_OPENAT => sys_openat(frame),
        syscall_number::SYS_CLOSE => sys_close(frame),
        syscall_number::SYS_PIPE2 => sys_pipe2(frame),
        syscall_number::SYS_GETDENTS64 => sys_getdents64(frame),
        syscall_number::SYS_LSEEK => sys_lseek(frame),
        syscall_number::SYS_FTRUNCATE => sys_ftruncate(frame),

        // I/O 操作 (Input/Output Operations)
        syscall_number::SYS_READ => sys_read(frame),
        syscall_number::SYS_WRITE => sys_write(frame),
        syscall_number::SYS_READV => sys_readv(frame),
        syscall_number::SYS_WRITEV => sys_writev(frame),
        syscall_number::SYS_PREAD64 => sys_pread64(frame),
        syscall_number::SYS_PWRITE64 => sys_pwrite64(frame),
        syscall_number::SYS_PREADV => sys_preadv(frame),
        syscall_number::SYS_PWRITEV => sys_pwritev(frame),
        syscall_number::SYS_SENDFILE => sys_sendfile(frame),
        syscall_number::SYS_PSELECT6 => sys_pselect6(frame),
        syscall_number::SYS_PPOLL => sys_ppoll(frame),

        // 文件元数据与同步 (File Metadata and Synchronization)
        syscall_number::SYS_READLINKAT => sys_readlinkat(frame),
        syscall_number::SYS_FSTATAT => sys_newfstatat(frame),
        syscall_number::SYS_FSTAT => sys_fstat(frame),
        syscall_number::SYS_SYNC => sys_sync(frame),
        syscall_number::SYS_FSYNC => sys_fsync(frame),
        syscall_number::SYS_FDATASYNC => sys_fdatasync(frame),

        // 定时器 (Timers)
        syscall_number::SYS_UTIMENSAT => sys_utimensat(frame),

        // 进程与控制 (Process and Control)
        syscall_number::SYS_EXIT => sys_exit(frame),
        syscall_number::SYS_EXIT_GROUP => sys_exit_group(frame),
        syscall_number::SYS_SET_TID_ADDRESS => sys_set_tid_address(frame),

        // 同步/休眠
        syscall_number::SYS_FUTEX => sys_futex(frame),
        syscall_number::SYS_SET_ROBUST_LIST => sys_set_robust_list(frame),
        syscall_number::SYS_GET_ROBUST_LIST => sys_get_robust_list(frame),
        syscall_number::SYS_NANOSLEEP => sys_nanosleep(frame),
        syscall_number::SYS_GETITIMER => sys_getitimmer(frame),
        syscall_number::SYS_SETITIMER => sys_setitimmer(frame),

        // POSIX 定时器 (POSIX Timers)
        syscall_number::SYS_CLOCK_SETTIME => sys_clock_settime(frame),
        syscall_number::SYS_CLOCK_GETTIME => sys_clock_gettime(frame),
        syscall_number::SYS_CLOCK_GETRES => sys_clock_getres(frame),
        syscall_number::SYS_SYSLOG => sys_syslog(frame),

        // 信号 (Signals)
        syscall_number::SYS_KILL => sys_kill(frame),
        syscall_number::SYS_TKILL => sys_tkill(frame),
        syscall_number::SYS_TGKILL => sys_tgkill(frame),
        syscall_number::SYS_SIGALTSTACK => sys_sigaltstack(frame),
        syscall_number::SYS_RT_SIGSUSPEND => sys_rt_sigsuspend(frame),
        syscall_number::SYS_RT_SIGACTION => sys_rt_sigaction(frame),
        syscall_number::SYS_RT_SIGPROCMASK => sys_rt_sigprocmask(frame),
        syscall_number::SYS_RT_SIGPENDING => sys_rt_sigpending(frame),
        syscall_number::SYS_RT_SIGTIMEDWAIT => sys_rt_sigtimedwait(frame),
        syscall_number::SYS_RT_SIGRETURN => sys_rt_sigreturn(frame),

        // 进程属性 (Process Attributes)
        syscall_number::SYS_REBOOT => sys_reboot(frame),
        syscall_number::SYS_SETGID => sys_setgid(frame),
        syscall_number::SYS_SETUID => sys_setuid(frame),
        syscall_number::SYS_SETRESUID => sys_setresuid(frame),
        syscall_number::SYS_GETRESUID => sys_getresuid(frame),
        syscall_number::SYS_SETRESGID => sys_setresgid(frame),
        syscall_number::SYS_GETRESGID => sys_getresgid(frame),
        syscall_number::SYS_SETPGID => sys_setpgid(frame),
        syscall_number::SYS_SETSID => sys_setsid(frame),

        // 系统信息 (System Information)
        syscall_number::SYS_UNAME => sys_uname(frame),
        syscall_number::SYS_SETHOSTNAME => sys_sethostname(frame),
        syscall_number::SYS_GETRLIMIT => sys_getrlimit(frame),
        syscall_number::SYS_SETRLIMIT => sys_setrlimit(frame),
        syscall_number::SYS_UMASK => sys_umask(frame),
        syscall_number::SYS_GETPID => sys_getpid(frame),
        syscall_number::SYS_GETPPID => sys_getppid(frame),
        syscall_number::SYS_GETPGID => sys_getpgid(frame),
        syscall_number::SYS_GETUID => sys_getuid(frame),
        syscall_number::SYS_GETEUID => sys_geteuid(frame),
        syscall_number::SYS_GETGID => sys_getgid(frame),
        syscall_number::SYS_GETEGID => sys_getegid(frame),
        syscall_number::SYS_GETTID => sys_gettid(frame),
        syscall_number::SYS_SYSINFO => sys_sysinfo(frame),

        // 网络 (Networking/Sockets)
        syscall_number::SYS_SOCKET => sys_socket(frame),
        syscall_number::SYS_BIND => sys_bind(frame),
        syscall_number::SYS_LISTEN => sys_listen(frame),
        syscall_number::SYS_ACCEPT => sys_accept(frame),
        syscall_number::SYS_CONNECT => sys_connect(frame),
        syscall_number::SYS_GETSOCKNAME => sys_getsockname(frame),
        syscall_number::SYS_GETPEERNAME => sys_getpeername(frame),
        syscall_number::SYS_SENDTO => sys_sendto(frame),
        syscall_number::SYS_RECVFROM => sys_recvfrom(frame),
        syscall_number::SYS_SETSOCKOPT => sys_setsockopt(frame),
        syscall_number::SYS_GETSOCKOPT => sys_getsockopt(frame),
        syscall_number::SYS_SHUTDOWN => sys_shutdown(frame),

        // 进程创建/执行 (Process Creation/Execution)
        syscall_number::SYS_CLONE => sys_clone(frame),
        syscall_number::SYS_EXECVE => sys_execve(frame),

        // 网络/I/O (续)
        syscall_number::SYS_ACCEPT4 => sys_accept4(frame),

        // 进程与控制 (续)
        syscall_number::SYS_WAIT4 => sys_wait4(frame),
        syscall_number::SYS_PRLIMIT64 => sys_prlimit(frame),

        // 内存管理 (Memory Management)
        syscall_number::SYS_BRK => sys_brk(frame),
        syscall_number::SYS_MUNMAP => sys_munmap(frame),
        syscall_number::SYS_MMAP => sys_mmap(frame),
        syscall_number::SYS_MPROTECT => sys_mprotect(frame),

        // 文件系统同步 (续)
        syscall_number::SYS_SYNCFS => sys_syncfs(frame),

        // 调度 (续)
        syscall_number::SYS_RENAMEAT2 => sys_renameat2(frame),

        // 随机数与内存文件
        syscall_number::SYS_GETRANDOM => sys_getrandom(frame),

        // 扩展文件元数据
        syscall_number::SYS_STATX => sys_statx(frame),

        // 获取网络接口地址列表 (非标准系统调用)
        syscall_number::SYS_GETIFADDRS => sys_getifaddrs(frame),

        // 扩展系统调用 (Extended/Legacy)
        // (send/recv 等已经通过更通用的接口实现，不需要单独分发)

        // 系统信息 (补充)
        syscall_number::SYS_SYSINFO => sys_sysinfo(frame),

        // POSIX 定时器 (补充)
        syscall_number::SYS_CLOCK_GETTIME => sys_clock_gettime(frame),
        syscall_number::SYS_CLOCK_SETTIME => sys_clock_settime(frame),
        syscall_number::SYS_CLOCK_GETRES => sys_clock_getres(frame),

        _ => {
            // 未知的系统调用
            frame.x10_a0 = (-(ENOSYS as isize)) as usize;
            crate::pr_debug!("Unknown syscall: {}", frame.x17_a7);
        }
    }
    crate::pr_debug!("syscall exit, return: {}", frame.x10_a0 as isize);
}

/// 宏：实现系统调用函数的自动包装器
///
/// 该宏会生成一个名为 `sys_name` 的函数，该函数签名固定为 `(frame: &mut TrapFrame)`
/// 1. 从TrapFrame中提取参数
/// 2. 调用对应的系统调用处理函数
/// 3. 将返回值写回TrapFrame
#[cfg(target_arch = "riscv64")]
#[macro_export]
macro_rules! impl_syscall {
    // noreturn, 6 args
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty, $t5:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::trap::TrapFrame) -> ! {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let a2 = frame.x12_a2 as $t2;
            let a3 = frame.x13_a3 as $t3;
            let a4 = frame.x14_a4 as $t4;
            let a5 = frame.x15_a5 as $t5;
            $kernel(a0, a1, a2, a3, a4, a5)
        }
    };

    // noreturn, 0..5 args (expanded)
    ($sys_name:ident, $kernel:path, noreturn, ()) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(_frame: &mut $crate::arch::trap::TrapFrame) -> ! {
            $kernel()
        }
    };
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) -> ! {
            let a0 = frame.x10_a0 as $t0;
            $kernel(a0)
        }
    };
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) -> ! {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            $kernel(a0, a1)
        }
    };
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty, $t2:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) -> ! {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let a2 = frame.x12_a2 as $t2;
            $kernel(a0, a1, a2)
        }
    };
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty, $t2:ty, $t3:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) -> ! {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let a2 = frame.x12_a2 as $t2;
            let a3 = frame.x13_a3 as $t3;
            $kernel(a0, a1, a2, a3)
        }
    };
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) -> ! {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let a2 = frame.x12_a2 as $t2;
            let a3 = frame.x13_a3 as $t3;
            let a4 = frame.x14_a4 as $t4;
            $kernel(a0, a1, a2, a3, a4)
        }
    };

    // returning, 6 args
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty, $t5:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let a2 = frame.x12_a2 as $t2;
            let a3 = frame.x13_a3 as $t3;
            let a4 = frame.x14_a4 as $t4;
            let a5 = frame.x15_a5 as $t5;
            let ret = $kernel(a0, a1, a2, a3, a4, a5);
            frame.x10_a0 = ret as isize as usize;
        }
    };

    // returning, 0..5 args (expanded)
    ($sys_name:ident, $kernel:path, ()) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let ret = $kernel();
            frame.x10_a0 = ret as isize as usize;
        }
    };
    ($sys_name:ident, $kernel:path, ($t0:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.x10_a0 as $t0;
            let ret = $kernel(a0);
            frame.x10_a0 = ret as isize as usize;
        }
    };
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let ret = $kernel(a0, a1);
            frame.x10_a0 = ret as isize as usize;
        }
    };
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let a2 = frame.x12_a2 as $t2;
            let ret = $kernel(a0, a1, a2);
            frame.x10_a0 = ret as isize as usize;
        }
    };
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty, $t3:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let a2 = frame.x12_a2 as $t2;
            let a3 = frame.x13_a3 as $t3;
            let ret = $kernel(a0, a1, a2, a3);
            frame.x10_a0 = ret as isize as usize;
        }
    };
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let a2 = frame.x12_a2 as $t2;
            let a3 = frame.x13_a3 as $t3;
            let a4 = frame.x14_a4 as $t4;
            let ret = $kernel(a0, a1, a2, a3, a4);
            frame.x10_a0 = ret as isize as usize;
        }
    };
}
