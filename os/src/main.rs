#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

#[macro_use]
mod console;
mod sbi;
mod config;
mod mm;
mod arch;
mod sync;

mod test;
use crate::test::{TEST_FAILED,FailedAssertion};
use core::arch::global_asm;
use core::hint;
use core::panic::PanicInfo;
use crate::arch::intr;
use crate::arch::trap;
use crate::arch::timer;
use core::sync::atomic::{Ordering};
use crate::sbi::shutdown;


/// 测试运行器。它由测试框架自动调用，并传入一个包含所有测试的切片。
fn test_runner(tests: &[&dyn Fn()]) {
    println!("\n\x1b[33m--- Running {} tests ---\x1b[0m", tests.len());

    // 重置失败计数器
    TEST_FAILED.store(0, Ordering::SeqCst);

    // 遍历并执行所有由 #[test_case] 注册的测试
    for test in tests {
        test();
    }

    let failed = TEST_FAILED.load(Ordering::SeqCst);
    println!("\x1b[33m\n--- Test Summary ---\x1b[0m");
    println!(
        "\x1b[33mTotal: {}\x1b[0m, \x1b[32mPassed: {}\x1b[0m, \x1b[91mFailed: {}\x1b[0m, \x1b[33mTests Finished\x1b[0m",
        tests.len(),
        tests.len() - failed,
        failed
    );
}

global_asm!(include_str!("entry.asm"));

#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    unsafe extern "C" {
        fn ekernel();
    }
    clear_bss();
    mm::init_frame_allocator(ekernel as usize, config::MEMORY_END);
    mm::init_heap();
    println!("Hello, world!");

    // 初始化工作
    trap::init();
    timer::init();
    unsafe { intr::enable_interrupts() };

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


test_case!(trivial_assertion,{
    kassert!(0 != 1);
});


test_case!(trivial_assertion2, {
    kassert!(1 != 1); // 这个测试会失败，但不会中断程序
});

test_case!(trivial_assertion3,{
    kassert!(21 != 1);
});