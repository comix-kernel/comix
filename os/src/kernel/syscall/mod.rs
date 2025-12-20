//! 系统调用模块
//!
//! 提供系统调用的实现

#![allow(dead_code)]
mod cred;
mod fcntl;
mod fs;
pub mod io;
mod ioctl;
mod ipc;
mod mm;
mod network;
mod signal;
mod sys;
mod task;
mod util;

use core::ffi::{c_char, c_int, c_uint, c_ulong, c_void};

use crate::{
    impl_syscall,
    uapi::{
        fs::LinuxStatFs,
        futex::RobustListHead,
        iovec::IoVec,
        resource::{Rlimit, Rusage},
        signal::{SigInfoT, SignalAction},
        sysinfo::SysInfo,
        time::{Itimerval, TimeSpec},
        types::{SigSetT, SizeT, StackT},
        uts_namespace::UtsNamespace,
    },
    vfs::Stat,
};
use cred::*;
use fcntl::*;
use fs::*;
use io::*;
use ioctl::*;
use ipc::*;
use mm::*;
use network::*;
use signal::*;
use sys::*;
use task::*;

// 系统调用实现注册
// 分类顺序与 arch/riscv/syscall/syscall_number.rs 保持一致

// 文件系统/目录操作 (Filesystem/Directory Operations)
impl_syscall!(sys_getcwd, getcwd, (*mut u8, usize));

// Epoll & Duplication
impl_syscall!(sys_dup, dup, (usize));
impl_syscall!(sys_dup3, dup3, (usize, usize, u32));
impl_syscall!(sys_fcntl, fcntl, (usize, i32, usize));
impl_syscall!(sys_ioctl, ioctl, (i32, u32, usize));

// 文件/目录创建与链接 (File/Directory Creation and Linking)
impl_syscall!(sys_mknodat, mknodat, (i32, *const c_char, u32, u64));
impl_syscall!(sys_mkdirat, mkdirat, (i32, *const c_char, u32));
impl_syscall!(sys_unlinkat, unlinkat, (i32, *const c_char, u32));
impl_syscall!(
    sys_symlinkat,
    symlinkat,
    (*const c_char, i32, *const c_char)
);

// 挂载/文件系统信息 (Mount/Filesystem Info)
impl_syscall!(sys_statfs, statfs, (*const c_char, *mut LinuxStatFs));

// 文件大小/权限/所有权 (File Size/Permissions/Ownership)
impl_syscall!(sys_faccessat, faccessat, (i32, *const c_char, i32, u32));
impl_syscall!(sys_chdir, chdir, (*const c_char));
impl_syscall!(sys_fchmodat, fchmodat, (i32, *const c_char, u32, u32));
impl_syscall!(sys_fchownat, fchownat, (i32, *const c_char, u32, u32, u32));

// 文件描述符操作 (File Descriptor Operations)
impl_syscall!(sys_openat, openat, (i32, *const c_char, u32, u32));
impl_syscall!(sys_close, close, (usize));
impl_syscall!(sys_pipe2, pipe2, (*mut i32, u32));
impl_syscall!(sys_getdents64, getdents64, (usize, *mut u8, usize));
impl_syscall!(sys_lseek, lseek, (usize, isize, usize));

// I/O 操作 (Input/Output Operations)
impl_syscall!(sys_read, read, (usize, *mut u8, usize));
impl_syscall!(sys_write, write, (usize, *const u8, usize));
impl_syscall!(sys_readv, readv, (usize, *const IoVec, usize));
impl_syscall!(sys_writev, writev, (usize, *const IoVec, usize));
impl_syscall!(sys_pread64, pread64, (usize, *mut u8, usize, i64));
impl_syscall!(sys_pwrite64, pwrite64, (usize, *const u8, usize, i64));
impl_syscall!(sys_preadv, preadv, (usize, *const IoVec, usize, i64));
impl_syscall!(sys_pwritev, pwritev, (usize, *const IoVec, usize, i64));
impl_syscall!(sys_sendfile, sendfile, (usize, usize, *mut i64, usize));
impl_syscall!(
    sys_pselect6,
    pselect6,
    (usize, usize, usize, usize, usize, usize)
);
impl_syscall!(sys_ppoll, ppoll, (usize, usize, usize, usize));

// 文件元数据与同步 (File Metadata and Synchronization)
impl_syscall!(
    sys_readlinkat,
    readlinkat,
    (i32, *const c_char, *mut u8, usize)
);
impl_syscall!(
    sys_newfstatat,
    newfstatat,
    (i32, *const c_char, *mut Stat, u32)
);
impl_syscall!(sys_fstat, fstat, (usize, *mut Stat));
impl_syscall!(sys_sync, sync, ());
impl_syscall!(sys_fsync, fsync, (usize));
impl_syscall!(sys_fdatasync, fdatasync, (usize));

// 定时器 (Timers)
impl_syscall!(
    sys_utimensat,
    utimensat,
    (i32, *const c_char, *const TimeSpec, u32)
);

// 进程与控制 (Process and Control)
impl_syscall!(sys_exit, exit, (c_int));
impl_syscall!(sys_exit_group, exit_group, noreturn, (c_int));
impl_syscall!(sys_set_tid_address, set_tid_address, (*mut c_int));

// 同步/休眠 (Synchronization/Sleeping)
impl_syscall!(sys_nanosleep, nanosleep, (*const TimeSpec, *mut TimeSpec));
impl_syscall!(
    sys_futex,
    futex,
    (*mut u32, c_int, u32, *const TimeSpec, *mut u32, u32)
);
impl_syscall!(
    sys_set_robust_list,
    set_robust_list,
    (*const RobustListHead, SizeT)
);
impl_syscall!(
    sys_get_robust_list,
    get_robust_list,
    (c_int, *mut *mut RobustListHead, *mut SizeT)
);
impl_syscall!(sys_getitimmer, getitimer, (c_int, *mut Itimerval));
impl_syscall!(
    sys_setitimmer,
    setitimer,
    (c_int, *const Itimerval, *mut Itimerval)
);

// POSIX 定时器 (POSIX Timers)
impl_syscall!(sys_clock_settime, clock_settime, (c_int, *const TimeSpec));
impl_syscall!(sys_clock_gettime, clock_gettime, (c_int, *mut TimeSpec));
impl_syscall!(sys_clock_getres, clock_getres, (c_int, *mut TimeSpec));
impl_syscall!(sys_syslog, syslog, (i32, *mut u8, i32));

// 信号 (Signals)
impl_syscall!(sys_kill, kill, (c_int, c_int));
impl_syscall!(sys_tkill, tkill, (c_int, c_int));
impl_syscall!(sys_tgkill, tgkill, (c_int, c_int, c_int));
impl_syscall!(sys_sigaltstack, signal_stack, (*const StackT, *mut StackT));
impl_syscall!(sys_rt_sigsuspend, rt_sigsuspend, (*const SigSetT, c_uint));
impl_syscall!(
    sys_rt_sigaction,
    rt_sigaction,
    (c_int, *const SignalAction, *mut SignalAction)
);
impl_syscall!(
    sys_rt_sigprocmask,
    rt_sigprocmask,
    (c_int, *const SigSetT, *mut SigSetT, c_uint)
);
impl_syscall!(sys_rt_sigpending, rt_sigpending, (*mut SigSetT, c_uint));
impl_syscall!(
    sys_rt_sigtimedwait,
    rt_sigtimedwait,
    (*const SigSetT, *mut SigInfoT, *const TimeSpec, c_uint)
);
impl_syscall!(sys_rt_sigreturn, rt_sigreturn, noreturn, ());

// 进程属性 (Process Attributes)
impl_syscall!(sys_reboot, reboot, (c_int, c_int, c_int, *mut c_void));
impl_syscall!(sys_setgid, setgid, (u32));
impl_syscall!(sys_setuid, setuid, (u32));
impl_syscall!(sys_setresuid, setresuid, (u32, u32, u32));
impl_syscall!(sys_getresuid, getresuid, (*mut u32, *mut u32, *mut u32));
impl_syscall!(sys_setresgid, setresgid, (u32, u32, u32));
impl_syscall!(sys_getresgid, getresgid, (*mut u32, *mut u32, *mut u32));
impl_syscall!(sys_setsid, setsid, ());
impl_syscall!(sys_setpgid, set_pgid, (c_int, c_int));

// 系统信息 (System Information)
impl_syscall!(sys_uname, uname, (*mut UtsNamespace));
impl_syscall!(sys_sethostname, set_hostname, (*const c_char, usize));
impl_syscall!(sys_getrlimit, getrlimit, (c_int, *mut Rlimit));
impl_syscall!(sys_setrlimit, setrlimit, (c_int, *const Rlimit));
impl_syscall!(sys_umask, umask, (u32));
impl_syscall!(sys_getpid, get_pid, ());
impl_syscall!(sys_getppid, get_ppid, ());
impl_syscall!(sys_getpgid, get_pgid, (c_int));
impl_syscall!(sys_getuid, getuid, ());
impl_syscall!(sys_geteuid, geteuid, ());
impl_syscall!(sys_getgid, getgid, ());
impl_syscall!(sys_getegid, getegid, ());
impl_syscall!(sys_gettid, gettid, ());
impl_syscall!(sys_sysinfo, sysinfo, (*mut SysInfo));

// 网络 (Networking/Sockets)
impl_syscall!(sys_socket, socket, (i32, i32, i32));
impl_syscall!(sys_bind, bind, (i32, *const u8, u32));
impl_syscall!(sys_listen, listen, (i32, i32));
impl_syscall!(sys_accept, accept, (i32, *mut u8, *mut u32));
impl_syscall!(sys_connect, connect, (i32, *const u8, u32));
impl_syscall!(sys_getsockname, getsockname, (i32, *mut u8, *mut u32));
impl_syscall!(sys_getpeername, getpeername, (i32, *mut u8, *mut u32));
impl_syscall!(
    sys_sendto,
    sendto,
    (i32, *const u8, usize, i32, *const u8, u32)
);
impl_syscall!(
    sys_recvfrom,
    recvfrom,
    (i32, *mut u8, usize, i32, *mut u8, *mut u32)
);
impl_syscall!(sys_setsockopt, setsockopt, (i32, i32, i32, *const u8, u32));
impl_syscall!(
    sys_getsockopt,
    getsockopt,
    (i32, i32, i32, *mut u8, *mut u32)
);

// 进程创建/执行 (Process Creation/Execution)
impl_syscall!(
    sys_clone,
    clone,
    (
        c_ulong,     // flags (a0)
        c_ulong,     // stack (a1)
        *mut c_int,  // parent_tid (a2)
        *mut c_void, // tls (a3)
        *mut c_int   // child_tid (a4)
    )
);
impl_syscall!(
    sys_execve,
    execve,
    (*const c_char, *const *const c_char, *const *const c_char)
);

// 网络/I/O (续)
impl_syscall!(sys_accept4, accept4, (i32, *mut u8, *mut u32, i32));

// 进程与控制 (续)
impl_syscall!(sys_wait4, wait4, (c_int, *mut c_int, c_int, *mut Rusage));
impl_syscall!(
    sys_prlimit,
    prlimit,
    (c_int, c_int, *const Rlimit, *mut Rlimit)
);

// 内存管理 (Memory Management)
impl_syscall!(sys_brk, brk, (usize));
impl_syscall!(sys_mmap, mmap, (*mut c_void, usize, i32, i32, i32, i64));
impl_syscall!(sys_munmap, munmap, (*mut c_void, usize));
impl_syscall!(sys_mprotect, mprotect, (*mut c_void, usize, i32));

// 文件系统同步 (续)
impl_syscall!(sys_syncfs, syncfs, (usize));

// 调度 (续)
impl_syscall!(
    sys_renameat2,
    renameat2,
    (i32, *const c_char, i32, *const c_char, u32)
);

// 挂载/文件系统操作 (Mount/Filesystem Operations)
impl_syscall!(sys_umount2, umount2, (*const c_char, i32));
impl_syscall!(
    sys_mount,
    mount,
    (
        *const c_char,
        *const c_char,
        *const c_char,
        u64,
        *const c_void
    )
);

// 随机数与内存文件
impl_syscall!(sys_getrandom, getrandom, (*mut c_void, SizeT, c_uint));

// 获取网络接口地址列表 (非标准系统调用)
impl_syscall!(sys_getifaddrs, getifaddrs, (*mut *mut u8));
impl_syscall!(sys_freeifaddrs, freeifaddrs, (*mut u8));

// 扩展系统调用 (Extended/Legacy)
impl_syscall!(sys_send, send, (i32, *const u8, usize, i32));
impl_syscall!(sys_recv, recv, (i32, *mut u8, usize, i32));
impl_syscall!(sys_seteuid, seteuid, (u32));
impl_syscall!(sys_setegid, setegid, (u32));
