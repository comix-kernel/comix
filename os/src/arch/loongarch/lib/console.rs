//! LoongArch64 控制台输出
//!
//! 使用 MMIO UART (NS16550) 实现串口输出

use core::fmt::{self, Write};

/// QEMU virt 平台 UART 物理基地址
const UART_PHYS_BASE: usize = 0x1fe001e0;

/// 通过 DMW0 映射的 UART 虚拟地址
/// DMW0: 0x8000_xxxx_xxxx_xxxx -> 物理地址 (uncached, 用于 MMIO)
const UART_BASE: usize = UART_PHYS_BASE | 0x8000_0000_0000_0000;

/// 标准输出
pub struct Stdout;

/// 标准输入
pub struct Stdin;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            unsafe {
                // 等待 UART 发送缓冲区空闲 (LSR bit 5, THRE)
                while ((UART_BASE + 5) as *const u8).read_volatile() & (1 << 5) == 0 {}
                // 写入 UART 数据寄存器
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
            // 等待 UART 接收缓冲区有数据 (LSR bit 0, DR)
            while ((UART_BASE + 5) as *const u8).read_volatile() & 1 == 0 {}
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
