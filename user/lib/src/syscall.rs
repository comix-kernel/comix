//! Syscall 接口封装

use core::ffi::c_char;

use crate::__syscall;
use crate::syscall;
use crate::syscall_numbers;

/// 关闭系统
pub fn shutdown() -> ! {
    syscall!(syscall_numbers::SYS_SHUTDOWN);
    unreachable!()
}

/// 退出进程
/// # 参数
/// - code: 退出状态码
pub fn exit(code: i32) -> ! {
    syscall!(syscall_numbers::SYS_EXIT, code);
    unreachable!()
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
pub unsafe fn write(fd: usize, buf: &[u8], count: usize) -> isize {
    syscall!(syscall_numbers::SYS_WRITE, fd, buf.as_ptr(), count)
}

/// 读取数据从控制台
/// # 参数
/// - fd: 文件描述符（通常为0表示标准输入）
/// - buf: 指向用于存储读取数据的缓冲区的指针
/// - count: 要读取的字节数
/// # 返回值
/// 读取的字节数，失败时返回负值
/// # Safety
/// 调用者必须确保 `buf` 指针有效且指向至少 `count
pub unsafe fn read(fd: usize, buf: &mut [u8], count: usize) -> isize {
    syscall!(syscall_numbers::SYS_READ, fd, buf.as_mut_ptr(), count)
}

/// 创建子进程
/// # 返回值
/// 子进程的PID（在父进程中）或0（在子进程中），失败时返回负值
pub fn fork() -> isize {
    syscall!(syscall_numbers::SYS_FORK)
}

/// 执行新程序
/// # 参数
/// - path: 要执行的程序的路径
/// - argv: 程序的命令行参数
/// - envp: 程序的环境变量
/// # 返回值
/// 成功时不返回，失败时返回负值
pub fn execve(
    path: *const c_char,
    argv: *const *const c_char,
    envp: *const *const c_char,
) -> isize {
    syscall!(syscall_numbers::SYS_EXEC, path, argv, envp)
}

/// 获取当前进程ID
/// # 返回值
/// 当前进程的PID
pub fn getpid() -> isize {
    syscall!(syscall_numbers::SYS_GETPID)
}

/// 等待子进程结束
/// # 参数
/// - pid: 要等待的子进程的PID
/// - status: 指向存储子进程退出状态的变量的指针
/// - options: 等待选项
/// # 返回值
/// 成功时返回子进程的PID，失败时返回负值
pub fn waitpid(pid: isize, status: *mut i32, options: usize) -> isize {
    syscall!(syscall_numbers::SYS_WAITPID, pid, status, options)
}

/// 打开文件
/// # 参数
/// - path: 要打开的文件路径
/// # 返回值
/// 成功时返回文件描述符，失败时返回负值
pub fn open(path: *const c_char) -> isize {
    syscall!(syscall_numbers::SYS_OPENAT, path)
}

/// 关闭文件
/// # 参数
/// - fd: 要关闭的文件描述符
/// # 返回值
/// 成功时返回0，失败时返回负值
pub fn close(fd: usize) -> isize {
    syscall!(syscall_numbers::SYS_CLOSE, fd)
}

/// 读取目录项
/// 向dirp指向的缓冲区中填充以NULL分割的文件名
/// # 参数
/// - fd: 目录的文件描述符
/// - dirp: 指向存储目录项的缓冲区的指针
/// - count: 要读取的字节数
/// # 返回值
/// 读取的字节数，失败时返回负值
/// # Safety
/// 调用者必须确保 `dirp` 指针有效且指向至少 `count` 字节的缓冲区
pub fn getdents(_fd: usize, _dirp: &mut [u8], _count: usize) -> isize {
    // syscall!(syscall_numbers::SYS_GETDENTS, fd, dirp.as_mut_ptr(), count)
    -1
}
