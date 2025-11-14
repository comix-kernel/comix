#![no_std]
#![no_main]

use core::ffi::CStr;
use lib::{
    execve, exit, fork,
    io::{print, read_line},
    shutdown, waitpid,
};

#[unsafe(no_mangle)]
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
                        CStr::from_bytes_with_nul(b"hello\0").unwrap().as_ptr(),
                        core::ptr::null(),
                    ];
                    execve(
                        CStr::from_bytes_with_nul(b"hello\0").unwrap().as_ptr(),
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
                let argv = [
                    CStr::from_bytes_with_nul(b"hello\0").unwrap().as_ptr(),
                    core::ptr::null(),
                ];
                execve(
                    CStr::from_bytes_with_nul(b"hello\0").unwrap().as_ptr(),
                    argv.as_ptr(),
                    core::ptr::null(),
                );
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
