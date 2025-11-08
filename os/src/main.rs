//! ComixOS - A RISC-V operating system kernel
//!
//! This is the main crate for ComixOS, an operating system kernel written in Rust
//! for RISC-V architecture. It provides basic OS functionalities including memory
//! management, process scheduling, and system call handling.

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

#[macro_use]
mod console;
mod arch;
mod config;
mod fs;
mod kernel;
mod mm;
mod sbi;
mod sync;
mod vfs;
#[macro_use]
mod test;
#[macro_use]
mod log;
use crate::arch::mm::vaddr_to_paddr;
use crate::arch::timer;
use crate::arch::trap;
use crate::kernel::rest_init;
use crate::sbi::shutdown;
use crate::test::run_early_tests;
use core::arch::global_asm;
use core::panic::PanicInfo;

/// 测试运行器。它由测试框架自动调用，并传入一个包含所有测试的切片。
#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    use crate::test::TEST_FAILED;
    use core::sync::atomic::Ordering;
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
    shutdown(false);
}

global_asm!(include_str!("entry.asm"));

/// Rust 内核主入口点
///
/// 这是从汇编代码跳转到的第一个 Rust 函数。它负责初始化内核的所有子系统,
/// 包括内存管理、中断处理、定时器和任务调度器。
///
/// # Safety
///
/// 此函数标记为 `#[unsafe(no_mangle)]` 以确保链接器可以找到它。
/// 它必须从正确初始化的汇编入口点调用。
#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    clear_bss();

    run_early_tests();

    // Initialize memory management (frame allocator + heap + kernel page table)
    mm::init();
    println!("Hello, world!");

    #[cfg(test)]
    test_main();

    // 初始化工作
    trap::init_boot_trap();
    timer::init();
    unsafe { arch::intr::enable_interrupts() };

    rest_init();
    unreachable!("Unreachable in rust_main()");
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

    let sbss_paddr = unsafe { vaddr_to_paddr(sbss as usize) };
    let ebss_paddr = unsafe { vaddr_to_paddr(ebss as usize) };

    (sbss_paddr..ebss_paddr).for_each(|a| unsafe {
        // 访问物理地址需要通过 paddr_to_vaddr 转换
        let va = crate::arch::mm::paddr_to_vaddr(a);
        (va as *mut u8).write_volatile(0)
    });
}

#[cfg(test)]
test_case!(trivial_assertion, {
    kassert!(0 != 1);
});

#[cfg(test)]
early_test!(exampe_early_test, {
    kassert!(1 == 1);
});

#[cfg(test)]
// 测试 `test_case!` 宏的 `(Interrupts)` 环境是否能正确地
// 在测试开始时启用中断，并在测试结束后恢复原始状态。
test_case!(verify_interrupt_environment, (Interrupts), {
    // 在这个代码块内部，中断应该已经被宏自动启用了。
    // 我们断言这一点来验证宏的行为。
    kassert!(crate::arch::intr::are_interrupts_enabled());

    println!("  -> Assertion passed: Interrupts are enabled.");

    // 为了让测试更有意义，我们可以手动禁用中断，
    // 然后验证 RAII 守卫是否会在测试结束时恢复它们。
    println!("  -> Manually disabling interrupts for demonstration...");
    unsafe {
        crate::arch::intr::disable_interrupts();
    }

    kassert!(!crate::arch::intr::are_interrupts_enabled());

    println!("  -> Assertion passed: Interrupts are now disabled manually.");
    println!("  -> Leaving test block, the guard should now restore the state...");
});

// 一个配套的测试，在 `(Interrupts)` 测试之后运行，
// 用来验证中断状态确实被恢复到了禁用状态。
test_case!(verify_interrupts_restored_after_test, {
    // 默认情况下，我们的测试运行器是在中断禁用的环境下运行的。
    // 如果前一个测试的 RAII 守卫工作正常，那么中断现在应该是禁用的。
    kassert!(!crate::arch::intr::are_interrupts_enabled());

    println!("  -> Assertion passed: Interrupts were correctly restored to disabled state.");
});
