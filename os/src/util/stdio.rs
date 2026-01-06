//! 标准输入输出工具模块

use core::fmt::{self, Write};

use alloc::string::String;
use alloc::vec::Vec;

pub struct Stdout;
pub struct Stdin;

pub fn console_putchar(c: usize) {
    crate::console::putchar(c as u8);
}

/// 使用 sbi 调用从控制台获取字符(qemu uart handler)
/// 返回值：字符的 ASCII 码
pub fn console_getchar() -> usize {
    crate::console::getchar().map_or(usize::MAX, |c| c as usize)
}

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        crate::console::write_str(s);
        Ok(())
    }
}

impl Stdin {
    pub fn read_char(&mut self) -> Option<char> {
        crate::console::getchar().map(|c| c as char)
    }

    pub fn read_line(&mut self, buf: &mut String) {
        let mut bytes = Vec::new();
        loop {
            if let Some(c) = crate::console::getchar() {
                if c == b'\n' || c == b'\r' {
                    break;
                }
                bytes.push(c);
            } else {
                break;
            }
        }
        buf.push_str(&String::from_utf8_lossy(&bytes));
    }
}

pub(crate) fn print(args: fmt::Arguments) {
    Stdout.write_fmt(args).unwrap();
}

pub fn stdin() -> Stdin {
    Stdin
}

/// 打印格式化文本到控制台
///
/// 这个宏类似于标准库的 `print!` 宏,但使用 SBI 调用将文本输出到控制台。
/// 它不会在末尾添加换行符。
///
/// # Examples
///
/// ```ignore
/// print!("Hello, world!");
/// print!("The answer is {}", 42);
/// ```
#[cfg(not(test))]
#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::util::stdio::print(format_args!($fmt $(, $($arg)+)?))
    }
}

/// 打印格式化文本到控制台并添加换行符
///
/// 这个宏类似于标准库的 `println!` 宏,但使用 SBI 调用将文本输出到控制台。
/// 它会在末尾自动添加换行符。
///
/// # Examples
///
/// ```ignore
/// println!("Hello, world!");
/// println!("The answer is {}", 42);
/// ```
#[cfg(not(test))]
#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::util::stdio::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}

#[cfg(test)]
#[macro_export]
/// 测试环境下的打印宏
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::arch::lib::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}

#[cfg(test)]
#[macro_export]
/// 测试环境下的打印宏
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::arch::lib::console::print(format_args!($fmt $(, $($arg)+)?))
    }
}
