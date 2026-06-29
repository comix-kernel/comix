//! Linux 系统调用号定义（架构无关）
//!
//! 系统调用号遵循 Linux 通用 ABI，在所有架构上数值一致。
//! 本模块仅包含内核实际处理的系统调用。

use core::usize;

// ---- 文件系统/目录操作 ----
pub const SYS_GETCWD: usize = 17;
pub const SYS_DUP: usize = 23;
pub const SYS_DUP3: usize = 24;
pub const SYS_FCNTL: usize = 25;
pub const SYS_IOCTL: usize = 29;
pub const SYS_MKNODAT: usize = 33;
pub const SYS_MKDIRAT: usize = 34;
pub const SYS_UNLINKAT: usize = 35;
pub const SYS_SYMLINKAT: usize = 36;
pub const SYS_LINKAT: usize = 37;
pub const SYS_MOUNT: usize = 40;
pub const SYS_UMOUNT2: usize = 39;
pub const SYS_STATFS: usize = 43;
pub const SYS_FACCESSAT: usize = 48;
pub const SYS_CHDIR: usize = 49;
pub const SYS_FCHMODAT: usize = 53;
pub const SYS_FCHOWNAT: usize = 54;
pub const SYS_OPENAT: usize = 56;
pub const SYS_CLOSE: usize = 57;
pub const SYS_PIPE2: usize = 59;
pub const SYS_GETDENTS64: usize = 61;
pub const SYS_LSEEK: usize = 62;
pub const SYS_FTRUNCATE: usize = 46;

// ---- I/O 操作 ----
pub const SYS_READ: usize = 63;
pub const SYS_WRITE: usize = 64;
pub const SYS_READV: usize = 65;
pub const SYS_WRITEV: usize = 66;
pub const SYS_PREAD64: usize = 67;
pub const SYS_PWRITE64: usize = 68;
pub const SYS_PREADV: usize = 69;
pub const SYS_PWRITEV: usize = 70;
pub const SYS_SENDFILE: usize = 71;

// ---- I/O 多路复用 ----
pub const SYS_PSELECT6: usize = 72;
pub const SYS_PPOLL: usize = 73;

// ---- 文件元数据与同步 ----
pub const SYS_READLINKAT: usize = 78;
pub const SYS_FSTATAT: usize = 79;
pub const SYS_FSTAT: usize = 80;
pub const SYS_SYNC: usize = 81;
pub const SYS_FSYNC: usize = 82;
pub const SYS_FDATASYNC: usize = 83;

// ---- 时间 ----
pub const SYS_UTIMENSAT: usize = 88;

// ---- 进程与控制 ----
pub const SYS_EXIT: usize = 93;
pub const SYS_EXIT_GROUP: usize = 94;
pub const SYS_SET_TID_ADDRESS: usize = 96;

// ---- 同步/休眠 ----
pub const SYS_FUTEX: usize = 98;
pub const SYS_SET_ROBUST_LIST: usize = 99;
pub const SYS_GET_ROBUST_LIST: usize = 100;
pub const SYS_NANOSLEEP: usize = 101;
pub const SYS_GETITIMER: usize = 102;
pub const SYS_SETITIMER: usize = 103;

// ---- 时钟 ----
pub const SYS_CLOCK_SETTIME: usize = 112;
pub const SYS_CLOCK_GETTIME: usize = 113;
pub const SYS_CLOCK_GETRES: usize = 114;
pub const SYS_CLOCK_NANOSLEEP: usize = 115;
pub const SYS_SYSLOG: usize = 116;

// ---- 调度 ----
pub const SYS_SCHED_SETPARAM: usize = 118;
pub const SYS_SCHED_SETSCHEDULER: usize = 119;
pub const SYS_SCHED_GETSCHEDULER: usize = 120;
pub const SYS_SCHED_GETPARAM: usize = 121;
pub const SYS_SCHED_SETAFFINITY: usize = 122;
pub const SYS_SCHED_GETAFFINITY: usize = 123;
pub const SYS_SCHED_YIELD: usize = 124;

// ---- 信号 ----
pub const SYS_KILL: usize = 129;
pub const SYS_TKILL: usize = 130;
pub const SYS_TGKILL: usize = 131;
pub const SYS_SIGALTSTACK: usize = 132;
pub const SYS_RT_SIGSUSPEND: usize = 133;
pub const SYS_RT_SIGACTION: usize = 134;
pub const SYS_RT_SIGPROCMASK: usize = 135;
pub const SYS_RT_SIGPENDING: usize = 136;
pub const SYS_RT_SIGTIMEDWAIT: usize = 137;
pub const SYS_RT_SIGRETURN: usize = 139;

// ---- 进程属性 ----
pub const SYS_REBOOT: usize = 142;
pub const SYS_SETGID: usize = 144;
pub const SYS_SETUID: usize = 146;
pub const SYS_SETRESUID: usize = 147;
pub const SYS_GETRESUID: usize = 148;
pub const SYS_SETRESGID: usize = 149;
pub const SYS_GETRESGID: usize = 150;
pub const SYS_TIMES: usize = 153;
pub const SYS_SETPGID: usize = 154;
pub const SYS_GETPGID: usize = 155;
pub const SYS_SETSID: usize = 157;
pub const SYS_UNAME: usize = 160;
pub const SYS_SETHOSTNAME: usize = 161;
pub const SYS_GETRLIMIT: usize = 163;
pub const SYS_SETRLIMIT: usize = 164;
pub const SYS_GETRUSAGE: usize = 165;
pub const SYS_UMASK: usize = 166;
pub const SYS_GETTIMEOFDAY: usize = 169;
pub const SYS_GETPID: usize = 172;
pub const SYS_GETPPID: usize = 173;
pub const SYS_GETUID: usize = 174;
pub const SYS_GETEUID: usize = 175;
pub const SYS_GETGID: usize = 176;
pub const SYS_GETEGID: usize = 177;
pub const SYS_GETTID: usize = 178;
pub const SYS_SYSINFO: usize = 179;

// ---- System V IPC ----
pub const SYS_SHMGET: usize = 194;
pub const SYS_SHMCTL: usize = 195;
pub const SYS_SHMAT: usize = 196;
pub const SYS_SHMDT: usize = 197;

// ---- 网络/Socket ----
pub const SYS_SOCKET: usize = 198;
pub const SYS_SOCKETPAIR: usize = 199;
pub const SYS_BIND: usize = 200;
pub const SYS_LISTEN: usize = 201;
pub const SYS_ACCEPT: usize = 202;
pub const SYS_CONNECT: usize = 203;
pub const SYS_GETSOCKNAME: usize = 204;
pub const SYS_GETPEERNAME: usize = 205;
pub const SYS_SENDTO: usize = 206;
pub const SYS_RECVFROM: usize = 207;
pub const SYS_SETSOCKOPT: usize = 208;
pub const SYS_GETSOCKOPT: usize = 209;
pub const SYS_SHUTDOWN: usize = 210;

// ---- 进程创建/执行 ----
pub const SYS_CLONE: usize = 220;
pub const SYS_EXECVE: usize = 221;

// ---- 内存管理 ----
pub const SYS_BRK: usize = 214;
pub const SYS_MUNMAP: usize = 215;
pub const SYS_MMAP: usize = 222;
pub const SYS_MPROTECT: usize = 226;
pub const SYS_MLOCK: usize = 228;
pub const SYS_MUNLOCK: usize = 229;
pub const SYS_MLOCKALL: usize = 230;
pub const SYS_MUNLOCKALL: usize = 231;
pub const SYS_MADVISE: usize = 233;

// ---- 网络 (续) ----
pub const SYS_ACCEPT4: usize = 242;

// ---- 进程与控制 (续) ----
pub const SYS_WAIT4: usize = 260;
pub const SYS_PRLIMIT64: usize = 261;

// ---- 文件系统 (续) ----
pub const SYS_SYNCFS: usize = 267;

// ---- 其他 ----
pub const SYS_RENAMEAT2: usize = 276;
pub const SYS_GETRANDOM: usize = 278;
pub const SYS_STATX: usize = 291;

// ---- 自定义内核扩展 ----
// 注意：此调用号在不同架构上可能不同
pub const SYS_GETIFADDRS: usize = crate::arch::abi::SYS_GETIFADDRS;
