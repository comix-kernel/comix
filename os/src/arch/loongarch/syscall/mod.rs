//! LoongArch64 系统调用模块

mod syscall_number;

pub use syscall_number::*;

use crate::arch::trap::TrapFrame;
use crate::kernel::syscall::*;
use crate::uapi::errno::ENOSYS;

/// 分发系统调用
pub fn dispatch_syscall(frame: &mut TrapFrame) {
    let syscall_id = frame.syscall_id();
    crate::pr_debug!(
        "syscall: {} args: [{:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x}]",
        syscall_id,
        frame.regs[4],
        frame.regs[5],
        frame.regs[6],
        frame.regs[7],
        frame.regs[8],
        frame.regs[9]
    );

    match syscall_id {
        // 文件系统/目录操作 (Filesystem/Directory Operations)
        SYS_GETCWD => sys_getcwd(frame),

        // Epoll & Duplication
        SYS_DUP => sys_dup(frame),
        SYS_DUP3 => sys_dup3(frame),
        SYS_FCNTL => sys_fcntl(frame),
        SYS_IOCTL => sys_ioctl(frame),

        // 文件/目录创建与链接 (File/Directory Creation and Linking)
        SYS_MKNODAT => sys_mknodat(frame),
        SYS_MKDIRAT => sys_mkdirat(frame),
        SYS_UNLINKAT => sys_unlinkat(frame),
        SYS_SYMLINKAT => sys_symlinkat(frame),

        // 挂载/文件系统信息 (Mount/Filesystem Info)
        SYS_MOUNT => sys_mount(frame),
        SYS_UMOUNT2 => sys_umount2(frame),
        SYS_STATFS => sys_statfs(frame),

        // 文件大小/权限/所有权 (File Size/Permissions/Ownership)
        SYS_FACCESSAT => sys_faccessat(frame),
        SYS_CHDIR => sys_chdir(frame),
        SYS_FCHMODAT => sys_fchmodat(frame),
        SYS_FCHOWNAT => sys_fchownat(frame),

        // 文件描述符操作 (File Descriptor Operations)
        SYS_OPENAT => sys_openat(frame),
        SYS_CLOSE => sys_close(frame),
        SYS_PIPE2 => sys_pipe2(frame),
        SYS_GETDENTS64 => sys_getdents64(frame),
        SYS_LSEEK => sys_lseek(frame),

        // I/O 操作 (Input/Output Operations)
        SYS_READ => sys_read(frame),
        SYS_WRITE => sys_write(frame),
        SYS_READV => sys_readv(frame),
        SYS_WRITEV => sys_writev(frame),
        SYS_PREAD64 => sys_pread64(frame),
        SYS_PWRITE64 => sys_pwrite64(frame),
        SYS_PREADV => sys_preadv(frame),
        SYS_PWRITEV => sys_pwritev(frame),
        SYS_SENDFILE => sys_sendfile(frame),
        SYS_PSELECT6 => sys_pselect6(frame),
        SYS_PPOLL => sys_ppoll(frame),

        // 文件元数据与同步 (File Metadata and Synchronization)
        SYS_READLINKAT => sys_readlinkat(frame),
        SYS_FSTATAT => sys_newfstatat(frame),
        SYS_FSTAT => sys_fstat(frame),
        SYS_SYNC => sys_sync(frame),
        SYS_FSYNC => sys_fsync(frame),
        SYS_FDATASYNC => sys_fdatasync(frame),

        // 定时器 (Timers)
        SYS_UTIMENSAT => sys_utimensat(frame),

        // 进程与控制 (Process and Control)
        SYS_EXIT => sys_exit(frame),
        SYS_EXIT_GROUP => sys_exit_group(frame),
        SYS_SET_TID_ADDRESS => sys_set_tid_address(frame),

        // 同步/休眠
        SYS_FUTEX => sys_futex(frame),
        SYS_SET_ROBUST_LIST => sys_set_robust_list(frame),
        SYS_GET_ROBUST_LIST => sys_get_robust_list(frame),
        SYS_NANOSLEEP => sys_nanosleep(frame),
        SYS_GETITIMER => sys_getitimmer(frame),
        SYS_SETITIMER => sys_setitimmer(frame),

        // POSIX 定时器 (POSIX Timers)
        SYS_CLOCK_SETTIME => sys_clock_settime(frame),
        SYS_CLOCK_GETTIME => sys_clock_gettime(frame),
        SYS_CLOCK_GETRES => sys_clock_getres(frame),
        SYS_SYSLOG => sys_syslog(frame),

        // 信号 (Signals)
        SYS_KILL => sys_kill(frame),
        SYS_TKILL => sys_tkill(frame),
        SYS_TGKILL => sys_tgkill(frame),
        SYS_SIGALTSTACK => sys_sigaltstack(frame),
        SYS_RT_SIGSUSPEND => sys_rt_sigsuspend(frame),
        SYS_RT_SIGACTION => sys_rt_sigaction(frame),
        SYS_RT_SIGPROCMASK => sys_rt_sigprocmask(frame),
        SYS_RT_SIGPENDING => sys_rt_sigpending(frame),
        SYS_RT_SIGTIMEDWAIT => sys_rt_sigtimedwait(frame),
        SYS_RT_SIGRETURN => sys_rt_sigreturn(frame),

        // 进程属性 (Process Attributes)
        SYS_REBOOT => sys_reboot(frame),
        SYS_SETGID => sys_setgid(frame),
        SYS_SETUID => sys_setuid(frame),
        SYS_SETRESUID => sys_setresuid(frame),
        SYS_GETRESUID => sys_getresuid(frame),
        SYS_SETRESGID => sys_setresgid(frame),
        SYS_GETRESGID => sys_getresgid(frame),
        SYS_SETPGID => sys_setpgid(frame),
        SYS_SETSID => sys_setsid(frame),

        // 系统信息 (System Information)
        SYS_UNAME => sys_uname(frame),
        SYS_SETHOSTNAME => sys_sethostname(frame),
        SYS_GETRLIMIT => sys_getrlimit(frame),
        SYS_SETRLIMIT => sys_setrlimit(frame),
        SYS_UMASK => sys_umask(frame),
        SYS_GETPID => sys_getpid(frame),
        SYS_GETPPID => sys_getppid(frame),
        SYS_GETPGID => sys_getpgid(frame),
        SYS_GETUID => sys_getuid(frame),
        SYS_GETEUID => sys_geteuid(frame),
        SYS_GETGID => sys_getgid(frame),
        SYS_GETEGID => sys_getegid(frame),
        SYS_GETTID => sys_gettid(frame),
        SYS_SYSINFO => sys_sysinfo(frame),

        // 网络 (Networking/Sockets)
        SYS_SOCKET => sys_socket(frame),
        SYS_BIND => sys_bind(frame),
        SYS_LISTEN => sys_listen(frame),
        SYS_ACCEPT => sys_accept(frame),
        SYS_CONNECT => sys_connect(frame),
        SYS_GETSOCKNAME => sys_getsockname(frame),
        SYS_GETPEERNAME => sys_getpeername(frame),
        SYS_SENDTO => sys_sendto(frame),
        SYS_RECVFROM => sys_recvfrom(frame),
        SYS_SETSOCKOPT => sys_setsockopt(frame),
        SYS_GETSOCKOPT => sys_getsockopt(frame),

        // 进程创建/执行 (Process Creation/Execution)
        SYS_CLONE => sys_clone(frame),
        SYS_EXECVE => sys_execve(frame),

        // 网络/I/O (续)
        SYS_ACCEPT4 => sys_accept4(frame),

        // 进程与控制 (续)
        SYS_WAIT4 => sys_wait4(frame),
        SYS_PRLIMIT64 => sys_prlimit(frame),

        // 内存管理 (Memory Management)
        SYS_BRK => sys_brk(frame),
        SYS_MUNMAP => sys_munmap(frame),
        SYS_MMAP => sys_mmap(frame),
        SYS_MPROTECT => sys_mprotect(frame),

        // 文件系统同步 (续)
        SYS_SYNCFS => sys_syncfs(frame),

        // 调度 (续)
        SYS_RENAMEAT2 => sys_renameat2(frame),

        // 随机数与内存文件
        SYS_GETRANDOM => sys_getrandom(frame),

        // 获取网络接口地址列表 (非标准系统调用)
        SYS_GETIFADDRS => sys_getifaddrs(frame),

        // 系统信息 (补充)
        SYS_SYSINFO => sys_sysinfo(frame),

        // POSIX 定时器 (补充)
        SYS_CLOCK_GETTIME => sys_clock_gettime(frame),
        SYS_CLOCK_SETTIME => sys_clock_settime(frame),
        SYS_CLOCK_GETRES => sys_clock_getres(frame),

        _ => {
            frame.set_syscall_ret((-ENOSYS) as usize);
        }
    }
    crate::pr_debug!("syscall exit, return: {}", frame.regs[4] as isize);
}

/// 宏：实现系统调用函数的自动包装器 (LoongArch 版)
///
/// 该宏会生成一个名为 `sys_name` 的函数，该函数签名固定为 `(frame: &mut TrapFrame)`
/// 1. 从TrapFrame中提取参数
/// 2. 调用对应的系统调用处理函数
/// 3. 将返回值写回TrapFrame
#[cfg(target_arch = "loongarch64")]
#[macro_export]
macro_rules! impl_syscall {
    // noreturn, 0 args
    ($sys_name:ident, $kernel:path, noreturn, ()) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(_frame: &mut $crate::arch::trap::TrapFrame) -> ! {
            $kernel()
        }
    };
    // noreturn, 1 arg
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) -> ! {
            let a0 = frame.regs[4] as $t0; // LoongArch a0 = $r4
            $kernel(a0)
        }
    };
    // noreturn, 2 args
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) -> ! {
            let a0 = frame.regs[4] as $t0;
            let a1 = frame.regs[5] as $t1;
            $kernel(a0, a1)
        }
    };

    // returning, 0 args
    ($sys_name:ident, $kernel:path, ()) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let ret = $kernel();
            frame.regs[4] = ret as isize as usize;
        }
    };
    // returning, 1 arg
    ($sys_name:ident, $kernel:path, ($t0:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.regs[4] as $t0;
            let ret = $kernel(a0);
            frame.regs[4] = ret as isize as usize;
        }
    };
    // returning, 2 args
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.regs[4] as $t0;
            let a1 = frame.regs[5] as $t1;
            let ret = $kernel(a0, a1);
            frame.regs[4] = ret as isize as usize;
        }
    };
    // returning, 3 args
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.regs[4] as $t0;
            let a1 = frame.regs[5] as $t1;
            let a2 = frame.regs[6] as $t2;
            let ret = $kernel(a0, a1, a2);
            frame.regs[4] = ret as isize as usize;
        }
    };
    // returning, 4 args
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty, $t3:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.regs[4] as $t0;
            let a1 = frame.regs[5] as $t1;
            let a2 = frame.regs[6] as $t2;
            let a3 = frame.regs[7] as $t3;
            let ret = $kernel(a0, a1, a2, a3);
            frame.regs[4] = ret as isize as usize;
        }
    };
    // returning, 5 args
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.regs[4] as $t0;
            let a1 = frame.regs[5] as $t1;
            let a2 = frame.regs[6] as $t2;
            let a3 = frame.regs[7] as $t3;
            let a4 = frame.regs[8] as $t4;
            let ret = $kernel(a0, a1, a2, a3, a4);
            frame.regs[4] = ret as isize as usize;
        }
    };
    // returning, 6 args
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty, $t5:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.regs[4] as $t0;
            let a1 = frame.regs[5] as $t1;
            let a2 = frame.regs[6] as $t2;
            let a3 = frame.regs[7] as $t3;
            let a4 = frame.regs[8] as $t4;
            let a5 = frame.regs[9] as $t5;
            let ret = $kernel(a0, a1, a2, a3, a4, a5);
            frame.regs[4] = ret as isize as usize;
        }
    };
}
