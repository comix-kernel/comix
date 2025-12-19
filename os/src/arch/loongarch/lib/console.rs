//! LoongArch64 控制台输出
//!
//! 使用 MMIO UART (NS16550) 实现串口输出

use core::fmt::{self, Write};

use super::super::constant::UART_BASE;

/// 标准输出
pub struct Stdout;

/// 标准输入
pub struct Stdin;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            unsafe {
                // 直接写入 UART 数据寄存器
                (UART_BASE as *mut u8).write_volatile(c);
            }
        }
        Ok(())
    }
}

impl Stdin {
    /// 从串口读取一个字符
    pub fn read_char(&mut self) -> char {
        unsafe {
            // 从 UART 数据寄存器读取
            (UART_BASE as *const u8).read_volatile() as char
        }
    }
}

/// 打印格式化内容
pub fn print(args: fmt::Arguments) {
    Stdout.write_fmt(args).unwrap();
}

/// 获取标准输入
pub fn stdin() -> Stdin {
    Stdin
}

/// 打印格式化文本到控制台 (不含换行)
#[macro_export]
macro_rules! earlyprint {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::arch::lib::console::print(format_args!($fmt $(, $($arg)+)?))
    }
}

/// 打印格式化文本到控制台 (含换行)
#[macro_export]
macro_rules! earlyprintln {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::arch::lib::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}
