//! 现在还没有兼容任何现有的操作系统ABI。
//! 编写的用户程序目前只支持通过 `syscall` 指令或我们自己封装的库进行系统调用。
//! 该模块定义了一个低级接口 `__syscall`，用于直接调用系统调用。

#![no_std]
#![allow(non_snake_case)]

pub use crate::syscalls::*;
use core::arch::global_asm;

global_asm!(include_str!("syscall.S"));

unsafe extern "C" {
    /// 低级系统调用接口
    /// 参数：
    /// - number: 系统调用号
    /// - ...: 可变数量的参数，最多6个
    ///   返回值：系统调用的返回值
    pub unsafe fn __syscall(number: usize, ...) -> isize;
}

/// 宏：简化系统调用的使用
/// Usage: syscall!(SYSCALL_NUM, [arg1, arg2, ...])
#[macro_export]
macro_rules! syscall {
    // 0 Arguments
    ($nr:expr) => ({
        let __sys_nr = $nr;
        // Safety: Calls assembly function (__syscall), which is marked unsafe.
        // We pass placeholders for unused arguments to align with the assembly stub's stack frame setup.
        unsafe {
            __syscall(__sys_nr, 0, 0, 0, 0, 0, 0)
        }
    });
    // 1 Argument
    ($nr:expr, $arg1:expr) => ({
        let __sys_nr = $nr;
        let __sys_arg1 = $arg1;
        unsafe {
            __syscall(__sys_nr, __sys_arg1, 0, 0, 0, 0, 0)
        }
    });
    // 2 Arguments
    ($nr:expr, $arg1:expr, $arg2:expr) => ({
        let __sys_nr = $nr;
        let __sys_arg1 = $arg1;
        let __sys_arg2 = $arg2;
        unsafe {
            __syscall(__sys_nr, __sys_arg1, __sys_arg2, 0, 0, 0, 0)
        }
    });
    // 3 Arguments
    ($nr:expr, $arg1:expr, $arg2:expr, $arg3:expr) => ({
        let __sys_nr = $nr;
        let __sys_arg1 = $arg1;
        let __sys_arg2 = $arg2;
        let __sys_arg3 = $arg3;
        unsafe {
            __syscall(__sys_nr, __sys_arg1, __sys_arg2, __sys_arg3, 0, 0, 0)
        }
    });
    // 4 Arguments
    ($nr:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr) => ({
        let __sys_nr = $nr;
        let __sys_arg1 = $arg1;
        let __sys_arg2 = $arg2;
        let __sys_arg3 = $arg3;
        let __sys_arg4 = $arg4;
        unsafe {
            __syscall(__sys_nr, __sys_arg1, __sys_arg2, __sys_arg3, __sys_arg4, 0, 0)
        }
    });
    // 5 Arguments
    ($nr:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr) => ({
        let __sys_nr = $nr;
        let __sys_arg1 = $arg1;
        let __sys_arg2 = $arg2;
        let __sys_arg3 = $arg3;
        let __sys_arg4 = $arg4;
        let __sys_arg5 = $arg5;
        unsafe {
            __syscall(__sys_nr, __sys_arg1, __sys_arg2, __sys_arg3, __sys_arg4, __sys_arg5, 0)
        }
    });
    // 6 Arguments
    ($nr:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr) => ({
        let __sys_nr = $nr;
        let __sys_arg1 = $arg1;
        let __sys_arg2 = $arg2;
        let __sys_arg3 = $arg3;
        let __sys_arg4 = $arg4;
        let __sys_arg5 = $arg5;
        let __sys_arg6 = $arg6;
        unsafe {
            __syscall(__sys_nr, __sys_arg1, __sys_arg2, __sys_arg3, __sys_arg4, __sys_arg5, __sys_arg6)
        }
    });
}

/// 系统调用号定义
pub mod syscall_numbers {
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
    // 其他系统调用号可以在这里继续添加
}
    
/// 封装的系统调用接口
mod syscalls {
    use crate::__syscall;
    use crate::syscall_numbers;
    use core::ffi::c_void;

    /// 退出进程
    /// # 参数
    /// - code: 退出状态码
    pub fn exit(code: i32) {
        syscall!(syscall_numbers::SYS_EXIT, code);
    }

    /// 写入数据到控制台
    /// # 参数
    /// - fd: 文件描述符（通常为1表示标准输出）
    /// - buf: 指向要写入的数据缓冲区的指针
    /// - count: 要写入的字节数
    /// # 返回值
    /// 写入的字节数，失败时返回负值
    /// # Safety
    /// 调用者必须确保 `buf` 指针有效且指向至少 `count` 字节的数据
    pub unsafe fn write(fd: usize, buf: *const c_void, count: usize) -> isize {
        syscall!(syscall_numbers::SYS_WRITE, fd, buf, count)
    }
}