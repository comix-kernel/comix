use core::sync::atomic::AtomicUsize;

#[derive(Copy, Clone)]
pub struct FailedAssertion {
    pub cond: &'static str,
    pub file: &'static str,
    pub line: u32,
}
pub static mut FAILED_LIST: [Option<FailedAssertion>; 32] = [None; 32];
pub const FAILED_LIST_CAPACITY: usize = 32;
pub static mut FAILED_INDEX: usize = 0;

pub static TEST_FAILED: AtomicUsize = AtomicUsize::new(0);

/// 一个不会 panic 的断言宏。它会记录打印失败状态，但不会中断程序。传入表达式，接受一个布尔值.
/// 只记录32个失败信息，超了不会记录
#[macro_export]
macro_rules! kassert {
    ($cond:expr) => {{
        if !$cond {
            $crate::test::TEST_FAILED.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
            unsafe {
                if $crate::test::FAILED_INDEX < $crate::test::FAILED_LIST_CAPACITY {
                    $crate::test::FAILED_LIST[$crate::test::FAILED_INDEX] =
                        Some($crate::test::FailedAssertion {
                            cond: stringify!($cond),
                            file: file!(),
                            line: line!(),
                        });
                    $crate::test::FAILED_INDEX += 1;
                } else {
                    println!(
                        "\x1b[31mWarning: FAILED_LIST overflow, assertion not recorded\x1b[0m"
                    );
                }
            }
        }
    }};
}
#[macro_export]
macro_rules! test_case {
    ($func:ident, $body:block) => {
        #[test_case]
        fn $func() {
            println!("\x1b[33m=======================================\x1b[0m");
            println!("\x1b[33mRunning test: {}::{}\x1b[0m", module_path!(), stringify!($func));

            let failed_before = $crate::test::TEST_FAILED.load(core::sync::atomic::Ordering::SeqCst);
            $body // 执行测试

            let failed_after = $crate::test::TEST_FAILED.load(core::sync::atomic::Ordering::SeqCst);
            let failed_count = failed_after - failed_before;

            unsafe {
                // 只打印本次测试失败的断言
                for i in failed_before..$crate::test::FAILED_INDEX {
                    if let Some(fail) = &$crate::test::FAILED_LIST[i] {
                        println!("\x1b[31mFailed assertion: {} at {}:{}\x1b[0m",
                        fail.cond, fail.file, fail.line);
                    }
                }
            }

            if failed_count == 0 {
                println!("\x1b[32m[ok] Test passed\x1b[0m\n");
            } else {
                println!("\x1b[91m[failed] Test failed with {} failed assertions\x1b[0m\n", failed_count);
            }
        }
    };
}
