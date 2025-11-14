#![no_std]
#![no_main]

use lib::{
    exit,
    io::{print, read_line},
};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    print(b"Hello, world!\n");
    print(b"$ ");

    let mut line = [0u8; 64];
    let cnt = read_line(&mut line);

    print(b"Hello, ");
    print(&line[..cnt]);
    print(b"!\n");
    exit(0)
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    exit(-1)
}
