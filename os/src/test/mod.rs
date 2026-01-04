mod guard;
pub mod macros;
pub mod net_test;
use crate::{
    arch::intr::{are_interrupts_enabled, disable_interrupts, enable_interrupts},
    earlyprintln,
};

/// 测试运行器。它由测试框架自动调用，并传入一个包含所有测试的切片。
#[cfg(test)]
pub fn test_runner(tests: &[&dyn Fn()]) {
    use crate::{arch::lib::sbi::shutdown, earlyprintln, test::macros::TEST_FAILED};
    use core::sync::atomic::Ordering;
    earlyprintln!("\n\x1b[33m--- Running {} tests ---\x1b[0m", tests.len());

    // 重置失败计数器
    TEST_FAILED.store(0, Ordering::SeqCst);

    // 遍历并执行所有由 #[test_case] 注册的测试
    for test in tests {
        test();
    }

    let failed = TEST_FAILED.load(Ordering::SeqCst);
    earlyprintln!("\x1b[33m\n--- Test Summary ---\x1b[0m");
    earlyprintln!(
        "\x1b[33mTotal: {}\x1b[0m, \x1b[32mPassed: {}\x1b[0m, \x1b[91mFailed: {}\x1b[0m, \x1b[33mTests Finished\x1b[0m",
        tests.len(),
        tests.len() - failed,
        failed
    );

    if failed > 0 {
        earlyprintln!("\x1b[91mSome tests failed!\x1b[0m");
        shutdown(true);
    } else {
        earlyprintln!("\x1b[32mAll tests passed!\x1b[0m");
        shutdown(false);
    }
}

pub fn run_early_tests() {
    // 从链接器脚本中定义的符号获取段的起始和结束地址
    unsafe extern "C" {
        static __early_test_start: extern "C" fn();
        static __early_test_end: extern "C" fn();
    }

    // 创建一个指向函数指针的切片
    // 安全性：我们假设链接器脚本正确创建了这些符号，并且它们对齐了函数指针。
    // 段中的每个项都是一个 `fn()` 类型的指针。
    let start = unsafe { &__early_test_start as *const _ as *const extern "C" fn() };
    let end = unsafe { &__early_test_end as *const _ as *const extern "C" fn() };

    // 计算测试数量
    let count = unsafe { end.offset_from(start) } as usize;
    if count == 0 {
        earlyprintln!("\x1b[36m[early_test] No early tests to run.\x1b[0m");
        return;
    }

    earlyprintln!(
        "\n\x1b[36m--- Running {} early tests (pre-mm) ---\x1b[0m",
        count
    );

    // 遍历并执行所有测试函数
    for i in 0..count {
        let test_fn = unsafe { *start.add(i) };
        test_fn();
    }

    earlyprintln!("\x1b[36m--- Early tests finished ---\x1b[0m\n");
}

/// 一个 RAII 守卫，用于在作用域内启用中断，并在离开作用域时恢复之前的状态。
///
/// # Safety
///
/// 创建此守卫需要调用 `enable_interrupts`，这是一个 `unsafe` 操作。
/// 因此，`new` 函数也是 `unsafe` 的。封装它的宏将负责处理安全性。
pub struct InterruptGuard {
    // 存储创建守卫之前的中断状态，以便恢复。
    // true = 已启用, false = 已禁用
    was_enabled: bool,
}

impl InterruptGuard {
    /// 创建一个新的守卫并启用中断。
    ///
    /// # Safety
    ///
    /// 调用者必须确保此时启用中断是安全的。例如，不能在持有自旋锁时调用。
    #[inline(always)]
    pub fn new() -> Self {
        // 读取当前中断状态,保存，如果是禁用的drop时会重新禁用
        let was_enabled = are_interrupts_enabled();
        // 启用中断
        unsafe {
            enable_interrupts();
        }
        Self { was_enabled }
    }
}

impl Drop for InterruptGuard {
    #[inline(always)]
    fn drop(&mut self) {
        if !self.was_enabled {
            // 如果之前是禁用的，就再次禁用。
            unsafe {
                disable_interrupts();
            }
        }
        // 如果之前是启用的，我们什么都不用做，因为中断本来就是开启的。
    }
}

#[cfg(test)]
mod tests {
    use crate::{early_test, kassert, test_case};

    test_case!(trivial_assertion, {
        kassert!(0 != 1);
    });

    early_test!(exampe_early_test, {
        kassert!(1 == 1);
    });

    // 测试 `test_case!` 宏的 `(Interrupts)` 环境是否能正确地
    // 在测试开始时启用中断，并在测试结束后恢复原始状态。
    // test_case!(verify_interrupt_environment, (Interrupt), {
    //     // 在这个代码块内部，中断应该已经被宏自动启用了。
    //     // 我们断言这一点来验证宏的行为。
    //     kassert!(crate::arch::intr::are_interrupts_enabled());

    //     println!("  -> Assertion passed: Interrupts are enabled.");

    //     // 为了让测试更有意义，我们可以手动禁用中断，
    //     // 然后验证 RAII 守卫是否会在测试结束时恢复它们。
    //     println!("  -> Manually disabling interrupts for demonstration...");
    //     unsafe {
    //         crate::arch::intr::disable_interrupts();
    //     }

    //     kassert!(!crate::arch::intr::are_interrupts_enabled());

    //     println!("  -> Assertion passed: Interrupts are now disabled manually.");
    //     println!("  -> Leaving test block, the guard should now restore the state...");
    // });

    // 一个配套的测试，在 `(Interrupts)` 测试之后运行，
    // 用来验证中断状态确实被恢复到了禁用状态。
    // test_case!(verify_interrupts_restored_after_test, {
    //     // 默认情况下，我们的测试运行器是在中断禁用的环境下运行的。
    //     // 如果前一个测试的 RAII 守卫工作正常，那么中断现在应该是禁用的。
    //     kassert!(!crate::arch::intr::are_interrupts_enabled());

    //     println!("  -> Assertion passed: Interrupts were correctly restored to disabled state.");
    // });
}
