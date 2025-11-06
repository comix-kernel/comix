#![no_std]
#![no_main]

use lib::{exit, write};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let buf = b"Hello, user space!\n";
    let count = buf.len();
    unsafe { write(1, buf, count) };
    exit(0)
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    exit(-1)
}
