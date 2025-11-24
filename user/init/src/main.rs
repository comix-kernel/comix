#![no_std]
#![no_main]

use lib::{
    execve, exit, fork,
    io::{print, read_line},
    shutdown, waitpid,
};

#[unsafe(no_mangle)]
/// # Safety
/// This is the entry point for the init process. Must be called by the kernel loader.
pub unsafe extern "C" fn _start() -> ! {
    let mut buf = [0u8; 1024];
    loop {
        print(b"$ ");
        let n = read_line(&mut buf);
        if n == 0 {
            continue;
        }
        let mut line = &buf[..n];
        if let Some(pos) = line.iter().position(|&b| b == b'\r' || b == b'\n') {
            line = &line[..pos];
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        // 取首个 token
        let cmd = line
            .split(|&b| b.is_ascii_whitespace())
            .next()
            .unwrap_or(&[]);
        match cmd {
            b"exit" => exit(0),
            b"bug1" => {
                if fork() == 0 {
                    print(b"Hello from child process!\n");
                    let argv = [
                        c"/home/user/bin/hello".as_ptr(),
                        core::ptr::null(),
                    ];
                    execve(
                        c"/home/user/bin/hello".as_ptr(),
                        argv.as_ptr(),
                        core::ptr::null(),
                    );
                } else {
                    print(b"Hello from parent process!\n");
                }
            }
            b"bug2" => {
                print(b"bug2\n");
                let id = fork();
                if id == 0 {
                    print(b"Hello from child process!\n");
                    exit(0);
                } else {
                    let mut s: i32 = 0;
                    waitpid(id, &mut s, 0);
                    print(b"Hello from parent process!\n");
                }
            }
            b"help" => print(b"Available commands: help, exit, bug1, bug2, shutdown, hello\n"),
            b"shutdown" => shutdown(),
            b"hello" => {
                // 使用 fork + execve 模式,避免替换 init 进程
                let pid = fork();
                if pid == 0 {
                    // 子进程: 执行 hello 程序
                    let argv = [
                        c"/home/user/bin/hello".as_ptr(),
                        core::ptr::null(),
                    ];
                    execve(
                        c"/home/user/bin/hello".as_ptr(),
                        argv.as_ptr(),
                        core::ptr::null(),
                    );
                    // 如果 execve 失败,退出子进程
                    print(b"Failed to execute hello\n");
                    exit(-1);
                } else {
                    // 父进程: 等待子进程完成
                    let mut status: i32 = 0;
                    waitpid(pid, &mut status, 0);
                }
            }
            b"fork" => {
                if fork() == 0 {
                    print(b"Hello from child process!\n");
                    exit(0);
                } else {
                    print(b"Hello from parent process!\n");
                }
            }
            _ => print(b"Unknown command\n"),
        }
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    exit(-1)
}
