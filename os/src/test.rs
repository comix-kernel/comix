//! HACK： 该模块下有几个变量不知道为什么被视作未使用，权宜之计
#![allow(dead_code)]
use core::sync::atomic::AtomicUsize;

#[derive(Copy, Clone)]
pub(crate) struct FailedAssertion {
    pub cond: &'static str,
    pub file: &'static str,
    pub line: u32,
}

// 添加公有构造函数，便于宏/其它代码显式构造
impl FailedAssertion {
    pub(crate) const fn new(cond: &'static str, file: &'static str, line: u32) -> Self {
        Self { cond, file, line }
    }
}

pub(crate) static mut FAILED_LIST: [Option<FailedAssertion>; 32] = [None; 32];
pub(crate) const FAILED_LIST_CAPACITY: usize = 32;
pub(crate) static mut FAILED_INDEX: usize = 0;

pub(crate) static TEST_FAILED: AtomicUsize = AtomicUsize::new(0);

// 把对 static mut 的不安全写操作封装到一个函数里（仅此处使用 unsafe）
pub(crate) fn record_failed_assertion(fa: FailedAssertion) {
    unsafe {
        if FAILED_INDEX < FAILED_LIST_CAPACITY {
            FAILED_LIST[FAILED_INDEX] = Some(fa);
            FAILED_INDEX += 1;
        } else {
            println!("\x1b[31mWarning: FAILED_LIST overflow, assertion not recorded\x1b[0m");
        }
    }
}

/// 一个不会 panic 的断言宏。它会记录打印失败状态，但不会中断程序。传入表达式，接受一个布尔值.
/// 只记录32个失败信息，超了不会记录
#[macro_export]
macro_rules! kassert {
    ($cond:expr) => {{
        if !$cond {
            $crate::test::TEST_FAILED.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
            // 在安全上下文中先构造值，避免在 unsafe 块内展开 metavariables
            let fa = $crate::test::FailedAssertion::new(stringify!($cond), file!(), line!());
            // 调用安全封装函数（内部负责 unsafe）
            $crate::test::record_failed_assertion(fa);
        }
    }};
}

/// 定义一个测试用例
///
/// 这个宏用于定义内核测试用例。它会自动处理测试的执行和结果报告,
/// 包括失败断言的详细信息和统计数据。
///
/// # Examples
///
/// ```ignore
/// test_case!(my_test, {
///     kassert!(1 + 1 == 2);
///     kassert!(true);
/// });
/// ```
///
/// # Implementation
///
/// 这个宏会：
/// - 创建一个带有 `#[test_case]` 属性的函数
/// - 记录测试前后的失败计数
/// - 打印测试名称和结果
/// - 显示详细的失败断言信息
#[macro_export]
macro_rules! test_case {
    ($func:ident, $body:block) => {
        #[doc = concat!("Test case: ", stringify!($func))]
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
