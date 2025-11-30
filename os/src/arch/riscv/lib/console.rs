use core::fmt::{self, Write};

use alloc::string::String;

use crate::arch::lib::sbi::console_putchar;

pub struct Stdout;
pub struct Stdin;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            console_putchar(c as usize);
        }
        Ok(())
    }
}

impl Stdin {
    // pub fn read_char(&mut self) -> char {
    //     let c = crate::arch::lib::sbi::console_getchar();
    //     c as u8 as char
    // }
    pub fn read_char(&mut self) -> char {
        let c = crate::arch::lib::sbi::console_getchar();
        // 立即回显字符（如果是可打印字符）
        if (c as u8) >= 0x20 && (c as u8) <= 0x7E {
            crate::arch::lib::sbi::console_putchar(c);
        } else if (c as u8) == b'\n' || (c as u8) == b'\r' {
            crate::arch::lib::sbi::console_putchar(b'\n' as usize);
        }
        c as u8 as char
    }

    pub fn read_line(&mut self, buf: &mut String) {
        loop {
            let c = self.read_char();
            if c == '\n' || c == '\r' {
                break;
            }
            buf.push(c);
        }
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
#[macro_export]
macro_rules! earlyprint {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::arch::lib::console::print(format_args!($fmt $(, $($arg)+)?))
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
#[macro_export]
macro_rules! earlyprintln {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::arch::lib::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}
