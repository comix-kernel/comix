//! 系统调用号定义
//!
//! 系统调用号遵循 Linux RISC-V 64 架构规范
#![allow(dead_code)]

// ========== 自定义系统调用（临时编号，避免冲突）==========
/// 关闭系统（自定义）
pub const SYS_SHUTDOWN: usize = 0;
/// 退出进程（临时，应使用 93）
pub const SYS_EXIT: usize = 93;
/// 创建子进程（临时，应使用 220）
pub const SYS_FORK: usize = 220;
/// 等待子进程结束（临时，应使用 260）
pub const SYS_WAITPID: usize = 260;
/// 获取当前进程ID（临时）
pub const SYS_GETPID: usize = 6;
/// 扩展数据段（堆）（临时）
pub const SYS_SBRK: usize = 7;
/// 休眠指定时间（毫秒）（临时）
pub const SYS_SLEEP: usize = 8;
/// 发送信号到进程（临时）
pub const SYS_KILL: usize = 9;
/// 执行新程序（临时，应使用 221）
pub const SYS_EXEC: usize = 221;

// ========== Linux RISC-V 64 标准系统调用 ==========
/// dup - 复制文件描述符
pub const SYS_DUP: usize = 23;
/// dup3 - 复制文件描述符到指定位置（带标志）
pub const SYS_DUP3: usize = 24;
/// openat - 相对于目录文件描述符打开文件
pub const SYS_OPENAT: usize = 56;
/// close - 关闭文件描述符
pub const SYS_CLOSE: usize = 57;
/// pipe2 - 创建管道（带标志）
pub const SYS_PIPE2: usize = 59;
/// getdents64 - 读取目录项（64位版本）
pub const SYS_GETDENTS64: usize = 61;
/// lseek - 修改文件偏移量
pub const SYS_LSEEK: usize = 62;
/// read - 从文件描述符读取
pub const SYS_READ: usize = 63;
/// write - 向文件描述符写入
pub const SYS_WRITE: usize = 64;
/// fstat - 获取文件状态
pub const SYS_FSTAT: usize = 80;
