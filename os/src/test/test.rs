//! HACK： 该模块下有几个变量不知道为什么被视作未使用，权宜之计
#![allow(dead_code)]
use crate::arch::intr::{are_interrupts_enabled, disable_interrupts, enable_interrupts, read_and_enable_interrupts, restore_interrupts};
use core::sync::atomic::AtomicUsize;
pub enum TestEnvironment {
    /// 在此测试期间启用中断
    Interrupts,
}
impl TestEnvironment {
    /// 根据环境创建一个guard，离开作用域时自动清理
    pub fn guard(&self) -> impl Drop {
        match self {
            // 对于中断环境，我们使用之前创建的 `with_interrupts!` 宏的内部实现
            TestEnvironment::Interrupts => {
                // `InterruptGuard` 的创建是 unsafe 的，但宏的使用者已经通过 `unsafe`
                // 块承担了责任，所以这里可以安全地调用。
                IntrGuardEnable::new()
            }
        }
    }
}

#[macro_export]
macro_rules! test_case {
    // 匹配带环境参数的版本: test_case!(name, (Env), { body })
    (
        $func_name:ident,
        ($env:ident),
        $body:block
    ) => {
        #[doc = concat!("Test case: ", stringify!($func_name), " with environment: ", stringify!($env))]
        #[test_case]
        fn $func_name() {
            println!("\x1b[33m=======================================\x1b[0m");
            println!(
                "\x1b[33mRunning test: {}::{} (with env: {})\x1b[0m",
                module_path!(),
                stringify!($func_name),
                stringify!($env)
            );

            let failed_before = $crate::test::TEST_FAILED.load(core::sync::atomic::Ordering::SeqCst);

            // 创建guard，它会在作用域结束时自动清理
            let _guard = $crate::test::TestEnvironment::$env.guard();

            $body // 执行测试

            let failed_after = $crate::test::TEST_FAILED.load(core::sync::atomic::Ordering::SeqCst);
            let failed_count = failed_after - failed_before;

            unsafe {
                for i in failed_before..$crate::test::FAILED_INDEX {
                    if let Some(fail) = &$crate::test::FAILED_LIST[i] {
                        println!(
                            "\x1b[31mFailed assertion: {} at {}:{}\x1b[0m",
                            fail.cond, fail.file, fail.line
                        );
                    }
                }
            }

            if failed_count == 0 {
                println!("\x1b[32m[ok] Test passed\x1b[0m\n");
            } else {
                println!(
                    "\x1b[91m[failed] Test failed with {} failed assertions\x1b[0m\n",
                    failed_count
                );
            }
        } // `_guard` 在这里离开作用域，环境被自动恢复
    };

    //下面是旧版本
    (
        $func_name:ident,
        $body:block
    ) => {
        #[doc = concat!("Test case: ", stringify!($func_name))]
        #[test_case]
        fn $func_name() {
            println!("\x1b[33m=======================================\x1b[0m");
            println!(
                "\x1b[33mRunning test: {}::{}\x1b[0m",
                module_path!(),
                stringify!($func_name)
            );

            let failed_before = $crate::test::TEST_FAILED.load(core::sync::atomic::Ordering::SeqCst);
            $body // 执行测试

            let failed_after = $crate::test::TEST_FAILED.load(core::sync::atomic::Ordering::SeqCst);
            let failed_count = failed_after - failed_before;

            unsafe {
                for i in failed_before..$crate::test::FAILED_INDEX {
                    if let Some(fail) = &$crate::test::FAILED_LIST[i] {
                        println!(
                            "\x1b[31mFailed assertion: {} at {}:{}\x1b[0m",
                            fail.cond, fail.file, fail.line
                        );
                    }
                }
            }

            if failed_count == 0 {
                println!("\x1b[32m[ok] Test passed\x1b[0m\n");
            } else {
                println!(
                    "\x1b[91m[failed] Test failed with {} failed assertions\x1b[0m\n",
                    failed_count
                );
            }
        }
    };
}
/// 一个 RAII 守卫，用于在作用域内启用中断，并在离开作用域时恢复之前的状态。
///
/// # Safety
///
/// 创建此守卫需要调用 `enable_interrupts`，这是一个 `unsafe` 操作。
/// 因此，`new` 函数也是 `unsafe` 的。封装它的宏将负责处理安全性。
pub struct IntrGuardEnable {
    // 存储创建守卫之前的中断状态，以便恢复。
    // true = 已启用, false = 已禁用
    flags: usize
}

impl IntrGuardEnable {
    /// 创建一个RAII守卫并启用中断。
    ///结构上与已经有的
    #[inline(always)]
    pub fn new() -> Self {
        // 读取当前中断状态,保存，如果是禁用的drop时会重新禁用
        let flags = unsafe { read_and_enable_interrupts() };
        IntrGuardEnable{ flags }
    }
}
///离开作用域自动恢复状态
impl Drop for IntrGuardEnable {
    #[inline(always)]
    fn drop(&mut self) {
        unsafe { restore_interrupts(self.flags) };
    }
}
