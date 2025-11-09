#![no_std]
#![no_main]

use lib::exit;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    exit(0)
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    exit(-1)
}

