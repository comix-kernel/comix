#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};

/// TODO: replace with proper heap allocator
/// Dummy allocator that always fails - placeholder until heap allocator is implemented
struct DummyAllocator;

unsafe impl GlobalAlloc for DummyAllocator {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        panic!("dealloc called on DummyAllocator");
    }
}

#[global_allocator]
static ALLOCATOR: DummyAllocator = DummyAllocator;

#[macro_use]
mod console;
mod sbi;
mod config;
mod mm;
mod arch;

use core::arch::global_asm;
use core::hint;
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
    trap::enable_interrupts();
    
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