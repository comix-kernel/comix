//! 系统调用模块
//!
//! 提供系统调用的实现

#![allow(dead_code)]
mod fs;
mod io;
mod ipc;
mod network;
mod signal;
mod sys;
mod task;
mod util;

use core::ffi::{c_char, c_int, c_void};

use crate::{
    impl_syscall,
    uapi::{
        fs::LinuxStatFs,
        resource::{Rlimit, Rusage},
        time::timespec,
    },
    vfs::Stat,
};
use fs::*;
use io::*;
use ipc::*;
use network::*;
use signal::*;
use sys::*;
use task::*;

// 系统调用实现注册
impl_syscall!(sys_reboot, reboot, (c_int, c_int, c_int, *mut c_void));
impl_syscall!(sys_exit_group, exit_group, noreturn, (c_int));
impl_syscall!(sys_write, write, (usize, *const u8, usize));
impl_syscall!(sys_read, read, (usize, *mut u8, usize));
impl_syscall!(sys_fork, fork, ());
impl_syscall!(
    sys_execve,
    execve,
    (*const u8, *const *const u8, *const *const u8)
);
impl_syscall!(sys_wait4, wait4, (c_int, *mut c_int, c_int, *mut Rusage));
impl_syscall!(sys_close, close, (usize));
impl_syscall!(sys_lseek, lseek, (usize, isize, usize));
impl_syscall!(sys_openat, openat, (i32, *const c_char, u32, u32));
impl_syscall!(sys_dup, dup, (usize));
impl_syscall!(sys_dup3, dup3, (usize, usize, u32));
impl_syscall!(sys_pipe2, pipe2, (*mut i32, u32));
impl_syscall!(sys_fstat, fstat, (usize, *mut Stat));
impl_syscall!(sys_getdents64, getdents64, (usize, *mut u8, usize));
impl_syscall!(sys_sethostname, set_hostname, (*const c_char, usize));
impl_syscall!(sys_getrlimit, getrlimit, (c_int, *mut Rlimit));
impl_syscall!(sys_setrlimit, setrlimit, (c_int, *const Rlimit));
impl_syscall!(
    sys_prlimit,
    prlimit,
    (c_int, c_int, *const Rlimit, *mut Rlimit)
);
impl_syscall!(sys_socket, socket, (i32, i32, i32));
impl_syscall!(sys_bind, bind, (i32, *const u8, u32));
impl_syscall!(sys_listen, listen, (i32, i32));
impl_syscall!(sys_accept, accept, (i32, *mut u8, *mut u32));
impl_syscall!(sys_connect, connect, (i32, *const u8, u32));
impl_syscall!(sys_send, send, (i32, *const u8, usize, i32));
impl_syscall!(sys_recv, recv, (i32, *mut u8, usize, i32));
impl_syscall!(sys_getifaddrs, getifaddrs, (*mut *mut u8));
impl_syscall!(sys_freeifaddrs, freeifaddrs, (*mut u8));
impl_syscall!(sys_ioctl, ioctl, (i32, u32, *mut u8));
impl_syscall!(sys_getpid, get_pid, ());
impl_syscall!(sys_getppid, get_ppid, ());
impl_syscall!(sys_exit, exit, (c_int));
impl_syscall!(sys_setsockopt, setsockopt, (i32, i32, i32, *const u8, u32));
impl_syscall!(
    sys_getsockopt,
    getsockopt,
    (i32, i32, i32, *mut u8, *mut u32)
);
impl_syscall!(sys_accept4, accept4, (i32, *mut u8, *mut u32, i32));
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
impl_syscall!(sys_getsockname, getsockname, (i32, *mut u8, *mut u32));
impl_syscall!(sys_getpeername, getpeername, (i32, *mut u8, *mut u32));
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
impl_syscall!(
    sys_utimensat,
    utimensat,
    (i32, *const c_char, *const timespec, u32)
);
impl_syscall!(
    sys_renameat2,
    renameat2,
    (i32, *const c_char, i32, *const c_char, u32)
);
impl_syscall!(sys_chdir, chdir, (*const c_char));
impl_syscall!(sys_getcwd, getcwd, (*mut u8, usize));
impl_syscall!(sys_mkdirat, mkdirat, (i32, *const c_char, u32));
impl_syscall!(sys_unlinkat, unlinkat, (i32, *const c_char, u32));
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
impl_syscall!(sys_statfs, statfs, (*const c_char, *mut LinuxStatFs));
impl_syscall!(sys_faccessat, faccessat, (i32, *const c_char, i32, u32));
impl_syscall!(sys_syslog, syslog, (i32, *mut u8, i32));
impl_syscall!(sys_nanosleep, nanosleep, (*const timespec, *mut timespec));
