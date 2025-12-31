//! 统一的控制台抽象
//!
//! 提供两阶段控制台：
//! - 早期阶段：使用 arch::sbi 直接输出
//! - 运行时阶段：使用 device::console::MAIN_CONSOLE

use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, Ordering};

use crate::sync::SpinLock;

/// 控制台是否已切换到运行时模式
static CONSOLE_RUNTIME: AtomicBool = AtomicBool::new(false);

/// 控制台锁（保护输出的原子性）
static CONSOLE_LOCK: SpinLock<()> = SpinLock::new(());

/// 切换到运行时控制台（设备初始化完成后调用）
pub fn init() {
    CONSOLE_RUNTIME.store(true, Ordering::Release);
}

/// 无锁的单字符输出（内部使用）
#[inline]
fn putchar_unlocked(c: u8) {
    if CONSOLE_RUNTIME.load(Ordering::Acquire) {
        // 运行时：使用 device console
        if let Some(console) = crate::device::console::MAIN_CONSOLE.read().as_ref() {
            let mut buf = [0u8; 4];
            let s = (c as char).encode_utf8(&mut buf);
            console.write_str(s);
        } else {
            // 降级到 SBI
            crate::arch::lib::sbi::console_putchar(c as usize);
        }
    } else {
        // 早期：使用 arch SBI
        crate::arch::lib::sbi::console_putchar(c as usize);
    }
}

/// 无锁的单字符输入（内部使用）
#[inline]
fn getchar_unlocked() -> Option<u8> {
    if CONSOLE_RUNTIME.load(Ordering::Acquire) {
        // 运行时：使用 device console
        if let Some(console) = crate::device::console::MAIN_CONSOLE.read().as_ref() {
            let ch = console.read_char();
            Some(ch as u8)
        } else {
            // 降级到 SBI
            let ch = crate::arch::lib::sbi::console_getchar();
            if ch == usize::MAX {
                None
            } else {
                Some(ch as u8)
            }
        }
    } else {
        // 早期：使用 arch SBI
        let ch = crate::arch::lib::sbi::console_getchar();
        if ch == usize::MAX {
            None
        } else {
            Some(ch as u8)
        }
    }
}

/// 带锁的字符串输出（公开接口）
pub fn write_str(s: &str) {
    let _guard = CONSOLE_LOCK.lock();
    for c in s.chars() {
        putchar_unlocked(c as u8);
    }
}

/// 带锁的单字符输出（公开接口，用于兼容性）
pub fn putchar(c: u8) {
    let _guard = CONSOLE_LOCK.lock();
    putchar_unlocked(c);
}

/// 带锁的单字符输入（公开接口）
pub fn getchar() -> Option<u8> {
    let _guard = CONSOLE_LOCK.lock();
    getchar_unlocked()
}

/// 控制台输出结构体（实现 Write trait，供日志系统使用）
pub struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // 在这里加锁，保护整个字符串的输出
        let _guard = CONSOLE_LOCK.lock();
        for c in s.chars() {
            putchar_unlocked(c as u8);
        }
        Ok(())
    }

    fn write_fmt(&mut self, args: fmt::Arguments) -> fmt::Result {
        // 重写 write_fmt 以确保整个格式化输出在一个锁内完成
        // 这样可以防止多个 CPU 的日志输出交错
        let _guard = CONSOLE_LOCK.lock();

        // 创建一个临时的 writer，它使用 putchar_unlocked（不加锁）
        struct UnlockedWriter;
        impl Write for UnlockedWriter {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                for c in s.chars() {
                    putchar_unlocked(c as u8);
                }
                Ok(())
            }
        }

        // 在持有锁的情况下格式化并输出
        UnlockedWriter.write_fmt(args)
    }
}
