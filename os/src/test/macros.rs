#![allow(dead_code)]
use crate::{
    arch::intr::{
        are_interrupts_enabled, disable_interrupts, enable_interrupts, read_and_enable_interrupts,
        restore_interrupts,
    },
    println,
};
use core::sync::atomic::{AtomicUsize, Ordering};

#[derive(Copy, Clone, Debug)]
pub struct FailedAssertion {
    pub cond: &'static str,
    pub file: &'static str,
    pub line: u32,
}

// 添加公有构造函数，便于宏/其它代码显式构造
impl FailedAssertion {
    pub const fn new(cond: &'static str, file: &'static str, line: u32) -> Self {
        Self { cond, file, line }
    }
}

pub static mut FAILED_LIST: [Option<FailedAssertion>; 32] = [None; 32];
pub const FAILED_LIST_CAPACITY: usize = 32;
pub static mut FAILED_INDEX: usize = 0;
pub static TEST_FAILED: AtomicUsize = AtomicUsize::new(0);
/// 安全地记录一个失败的断言。
///
/// 此函数是 `kassert!` 宏的后端，负责处理 unsafe 的全局可变状态访问。
/// 它会原子地更新失败列表，防止并发访问时的数据竞争。
///
/// # Arguments
///
/// * `assertion`: 要记录的 `FailedAssertion` 实例。
pub fn record_failed_assertion(assertion: FailedAssertion) {
    let index = TEST_FAILED.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
    if index < FAILED_LIST_CAPACITY {
        unsafe {
            FAILED_LIST[index] = Some(assertion);
            if index + 1 > FAILED_INDEX {
                FAILED_INDEX = index + 1;
            }
        }
    } else {
        println!(
            "\x1b[91m[warn] Failed assertion list is full (capacity {}). Cannot record: {}\x1b[0m",
            FAILED_LIST_CAPACITY, assertion.cond
        );
    }
}

/// 判断条件是否为真，如果为假则记录一个失败的断言。
#[macro_export]
macro_rules! kassert {
    ($cond:expr) => {{
        if !$cond {
            // 使用 $crate 确保宏在任何模块中都能正确找到路径
            // 在安全上下文中先构造值，避免在 unsafe 块内展开 metavariables
            let fa =
                $crate::test::macros::FailedAssertion::new(stringify!($cond), file!(), line!());
            // 调用安全封装函数（内部负责 unsafe）
            $crate::test::macros::record_failed_assertion(fa);
        }
    }};
}

/// 定义一个早期测试（在内存管理等核心服务初始化前运行）。
///
/// 测试函数会被放入一个自定义的链接段 `.early_test_entry` 中，
/// 以便在启动早期被统一执行。
#[macro_export]
macro_rules! early_test {
    // 匹配一个函数名和一个代码块
    ($func_name:ident,$body:block) => {
        // 使用 paste::paste! 来组合标识符，确保函数名唯一
        paste::paste! {
            // 生成一个内部函数，避免命名冲突
            #[doc = concat!("Early test case: ", stringify!($func_name))]
            #[allow(dead_code)] // 函数不是直接调用的，所以允许未使用
            fn [<early_test_ $func_name>]() {
                // 使用早期打印，避免依赖 MAIN_CONSOLE 初始化
                $crate::earlyprintln!(
                    "\x1b[36m[early_test] Running: {}\x1b[0m\n",
                    stringify!($func_name)
                );
                $body
                $crate::earlyprintln!(
                    "\x1b[36m[early_test] Passed: {}\x1b[0m\n",
                    stringify!($func_name)
                );
            }

            // 将函数指针放入自定义的链接器段
            #[used] // 确保编译器不会优化掉这个静态变量
            #[unsafe(link_section = ".early_test_entry")]
            static [<EARLY_TEST_ENTRY_ $func_name:upper>]: fn() = [<early_test_$func_name>];
        }
    };
}
/// 定义一个标准的测试用例。
///
/// 提供两种语法：
/// 1. 带环境：`test_case!(test_name, env_variable, { code });`
/// 2. 不带环境：`test_case!(test_name, { code });`
#[macro_export]
macro_rules! test_case {
    (
        $func_name:ident,
        ($env:ident),
        $body:block
    )  => {
        #[cfg(test)]
        #[test]
        fn $func_name() {

            let _guard = $crate::test::guard::TestEnvGuard::enter($env);

            println!("\x1b[33m[Running test: {} (with env)]\x1b[0m", stringify!($name));
            let failed_before = $crate::test::macros::TEST_FAILED.load(core::sync::atomic::Ordering::SeqCst);

            $body

            let failed_after = $crate::test::macros::TEST_FAILED.load(core::sync::atomic::Ordering::SeqCst);
            if failed_after == failed_before {
                println!("\x1b[32m[ok] {}\x1b[0m", stringify!($name));
            } else {
                println!("\x1b[91m[failed] {}\x1b[0m", stringify!($name));
            }
        }
    };
    (
        $func_name:ident,
        $body:block
    ) => {
        #[doc = concat!("Test case: ", stringify!($func_name))]
        #[test_case]
        fn $func_name() {
            $crate::println!("\x1b[33m=======================================\x1b[0m");
            $crate::println!(
                "\x1b[33mRunning test: {}::{}\x1b[0m",
                module_path!(),
                stringify!($func_name)
            );

            let failed_before = $crate::test::macros::TEST_FAILED.load(core::sync::atomic::Ordering::SeqCst);

            $body

            let failed_after = $crate::test::macros::TEST_FAILED.load(core::sync::atomic::Ordering::SeqCst);
            let failed_count = failed_after - failed_before;

            unsafe {
                for i in failed_before..$crate::test::macros::FAILED_INDEX {
                    if let Some(fail) = $crate::test::macros::FAILED_LIST[i] {
                        $crate::println!(
                            "\x1b[31mFailed assertion: {} at {}:{}\x1b[0m",
                            fail.cond, fail.file, fail.line
                        );
                    }
                }
            }

            if failed_count == 0 {
                $crate::println!("\x1b[32m[ok] Test passed\x1b[0m\n");
            } else {
                $crate::println!(
                    "\x1b[91m[failed] Test failed with {} failed assertions\x1b[0m\n",
                    failed_count
                );
            }
        }
    };
}
//test_case!的普通函数
fn run_test(test_name: &str, env_name: Option<&str>, test_fn: impl FnOnce()) {
    println!("\x1b[33m=======================================\x1b[0m");
    if let Some(env) = env_name {
        println!(
            "\x1b[33mRunning test: {}::{} (with env: {})\x1b[0m",
            module_path!(),
            test_name,
            env
        );
    } else {
        println!(
            "\x1b[33mRunning test: {}::{}\x1b[0m",
            module_path!(),
            test_name
        );
    }

    let failed_before = TEST_FAILED.load(core::sync::atomic::Ordering::SeqCst);

    // 执行测试函数
    test_fn();

    let failed_after = TEST_FAILED.load(core::sync::atomic::Ordering::SeqCst);
    let failed_count = failed_after - failed_before;

    unsafe {
        for i in failed_before..FAILED_INDEX {
            if let Some(fail) = &FAILED_LIST[i] {
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

//early_test!的执行函数
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
        println!("\x1b[36m[early_test] No early tests to run.\x1b[0m");
        return;
    }

    println!(
        "\n\x1b[36m--- Running {} early tests (pre-mm) ---\x1b[0m",
        count
    );

    // 遍历并执行所有测试函数
    for i in 0..count {
        let test_fn = unsafe { *start.add(i) };
        test_fn();
    }

    println!("\x1b[36m--- Early tests finished ---\x1b[0m\n");
}
