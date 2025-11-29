//! RISC-V 架构的系统调用分发模块
use crate::kernel::syscall::*;
use crate::pr_err;

mod syscall_number;

/// 分发系统调用
pub fn dispatch_syscall(frame: &mut super::trap::TrapFrame) {
    match frame.x17_a7 {
        syscall_number::SYS_REBOOT => sys_reboot(frame),
        syscall_number::SYS_GETPID => sys_getpid(frame),
        syscall_number::SYS_GETPPID => sys_getppid(frame),
        syscall_number::SYS_EXIT => sys_exit(frame),
        syscall_number::SYS_EXIT_GROUP => sys_exit_group(frame),
        syscall_number::SYS_WRITE => sys_write(frame),
        syscall_number::SYS_READ => sys_read(frame),
        syscall_number::SYS_CLONE => sys_clone(frame),
        syscall_number::SYS_EXECVE => sys_execve(frame),
        syscall_number::SYS_WAIT4 => sys_wait4(frame),
        syscall_number::SYS_DUP => sys_dup(frame),
        syscall_number::SYS_DUP3 => sys_dup3(frame),
        syscall_number::SYS_OPENAT => sys_openat(frame),
        syscall_number::SYS_CLOSE => sys_close(frame),
        syscall_number::SYS_PIPE2 => sys_pipe2(frame),
        syscall_number::SYS_GETDENTS64 => sys_getdents64(frame),
        syscall_number::SYS_LSEEK => sys_lseek(frame),
        syscall_number::SYS_FSTAT => sys_fstat(frame),
        syscall_number::SYS_GETIFADDRS => sys_getifaddrs(frame),
        syscall_number::SYS_SETSOCKOPT => sys_setsockopt(frame),
        syscall_number::SYS_SOCKET => sys_socket(frame),
        syscall_number::SYS_BIND => sys_bind(frame),
        syscall_number::SYS_LISTEN => sys_listen(frame),
        syscall_number::SYS_ACCEPT => sys_accept(frame),
        syscall_number::SYS_CONNECT => sys_connect(frame),
        syscall_number::SYS_SENDTO => sys_send(frame),
        syscall_number::SYS_RECVFROM => sys_recv(frame),
        syscall_number::SYS_GETSOCKOPT => sys_getsockopt(frame),
        syscall_number::SYS_SETHOSTNAME => sys_sethostname(frame),
        syscall_number::SYS_GETRLIMIT => sys_getrlimit(frame),
        syscall_number::SYS_SETRLIMIT => sys_setrlimit(frame),
        syscall_number::SYS_PRLIMIT64 => sys_prlimit(frame),
        syscall_number::SYS_NANOSLEEP => sys_nanosleep(frame),
        syscall_number::SYS_SYNC => sys_sync(frame),
        syscall_number::SYS_SYNCFS => sys_syncfs(frame),
        syscall_number::SYS_FSYNC => sys_fsync(frame),
        syscall_number::SYS_FDATASYNC => sys_fdatasync(frame),
        syscall_number::SYS_RT_SIGPENDING => sys_rt_sigpending(frame),
        syscall_number::SYS_RT_SIGPROCMASK => sys_rt_sigprocmask(frame),
        syscall_number::SYS_RT_SIGACTION => sys_rt_sigaction(frame),
        syscall_number::SYS_RT_SIGTIMEDWAIT => sys_rt_sigtimedwait(frame),
        syscall_number::SYS_RT_SIGSUSPEND => sys_rt_sigsuspend(frame),
        syscall_number::SYS_RT_SIGRETURN => sys_rt_sigreturn(frame),
        syscall_number::SYS_SIGALTSTACK => sys_sigaltstack(frame),
        syscall_number::SYS_KILL => sys_kill(frame),
        syscall_number::SYS_TKILL => sys_tkill(frame),
        syscall_number::SYS_TGKILL => sys_tgkill(frame),
        syscall_number::SYS_UNAME => sys_uname(frame),
        syscall_number::SYS_GETTID => sys_gettid(frame),
        syscall_number::SYS_SYSINFO => sys_sysinfo(frame),
        _ => {
            // 未知的系统调用
            frame.x10_a0 = (-2isize) as usize; // -ENOSYS
            pr_err!("Unknown syscall: {}", frame.x17_a7);
        }
    }
}

/// 宏：实现系统调用函数的自动包装器
///
/// 该宏会生成一个名为 `sys_name` 的函数，该函数签名固定为 `(frame: &mut TrapFrame)`
/// 1. 从TrapFrame中提取参数
/// 2. 调用对应的系统调用处理函数
/// 3. 将返回值写回TrapFrame
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
