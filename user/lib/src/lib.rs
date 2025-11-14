//! 现在还没有兼容任何现有的操作系统ABI。
//! 编写的用户程序目前只支持通过 `syscall` 指令或我们自己封装的库进行系统调用。
//! 该模块定义了一个低级接口 `__syscall`，用于直接调用系统调用。

#![no_std]
#![allow(non_snake_case)]
pub mod io;
mod syscall;
pub mod syscall_numbers;

pub use crate::syscall::*;
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
