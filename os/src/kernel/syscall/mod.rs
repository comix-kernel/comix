//! 系统调用模块
//!
//! 提供系统调用的实现
#![allow(dead_code)]

use riscv::register::sstatus;

use crate::impl_syscall;

/// 关闭系统调用
fn shutdown() -> ! {
    crate::shutdown(false);
}

/// TODO: 进程退出系统调用
fn exit(_code: i32) -> ! {
    crate::shutdown(false);
}

fn write(fd: usize, buf: *const u8, count: usize) -> isize {
    if fd == 1 {
        unsafe { sstatus::set_sum() };
        for i in 0..count {
            let c = unsafe { *buf.add(i) };
            crate::sbi::console_putchar(c as usize);
        }
        unsafe { sstatus::clear_sum() };
        count as isize
    } else {
        -1 // 不支持其他文件描述符
    }
}

// 系统调用实现注册
impl_syscall!(sys_shutdown, shutdown, noreturn, ());
impl_syscall!(sys_exit, exit, noreturn, (i32));
impl_syscall!(sys_write, write, (usize, *const u8, usize));
