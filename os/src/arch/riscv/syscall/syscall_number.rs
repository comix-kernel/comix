//! 系统调用号定义
#![allow(dead_code)]
/// 关机
pub const SYS_SHUTDOWN: usize = 0;
/// 退出进程
pub const SYS_EXIT: usize = 1;
/// 打印字符串到控制台
pub const SYS_WRITE: usize = 2;
/// 读取数据从控制台
pub const SYS_READ: usize = 3;
/// 创建子进程
pub const SYS_FORK: usize = 4;
/// 等待子进程结束
pub const SYS_WAITPID: usize = 5;
/// 获取当前进程ID
pub const SYS_GETPID: usize = 6;
/// 扩展数据段（堆）
pub const SYS_SBRK: usize = 7;
/// 休眠指定时间（毫秒）
pub const SYS_SLEEP: usize = 8;
/// 发送信号到进程
pub const SYS_KILL: usize = 9;
/// 执行新程序
pub const SYS_EXEC: usize = 10;
