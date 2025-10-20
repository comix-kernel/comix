#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

#[macro_use]
mod console;
mod sbi;
mod config;
mod mm;
mod arch;

use core::arch::global_asm;
use core::panic::PanicInfo;
use crate::arch::trap;
use crate::arch::timer;
use crate::sbi::shutdown;

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    shutdown(false);
}

global_asm!(include_str!("entry.asm"));

#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    clear_bss();
    println!("Hello, world!");

    // 初始化工作
    trap::init();
    timer::init();
    
    #[cfg(test)]
    test_main();
    
    shutdown(false)
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        println!(
            "Panicked at {}:{} {}",
            location.file(),
            location.line(),
            info.message()
        );
    } else {
        println!("Panicked: {}", info.message());
    }
    shutdown(true)
}

fn clear_bss() {
    unsafe extern "C" {
        fn sbss();
        fn ebss();
    }

    (sbss as usize..ebss as usize).for_each(|a| {
        unsafe { (a as *mut u8).write_volatile(0) }
    });
}

#[test_case]
fn trivial_assertion() {
    print!("Testing trivial assertion...");
    assert_ne!(0, 1);
}