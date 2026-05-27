//! 统一的控制台抽象
//!
//! 提供两阶段控制台：
//! - 早期阶段：使用 arch::sbi 直接输出
//! - 运行时阶段：使用 device::console::MAIN_CONSOLE

use core::cell::UnsafeCell;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::arch::Platform;
use crate::sync::SpinLock;

/// 控制台是否已切换到运行时模式
static CONSOLE_RUNTIME: AtomicBool = AtomicBool::new(false);

/// 控制台锁（保护输出的原子性）
static CONSOLE_LOCK: SpinLock<()> = SpinLock::new(());

const BOOT_CONSOLE_BUFFER_SIZE: usize = 16 * 1024;

struct BootConsoleBuffer {
    write_seq: AtomicUsize,
    replayed_seq: AtomicUsize,
    bytes: [UnsafeCell<u8>; BOOT_CONSOLE_BUFFER_SIZE],
}

unsafe impl Sync for BootConsoleBuffer {}

impl BootConsoleBuffer {
    const fn new() -> Self {
        Self {
            write_seq: AtomicUsize::new(0),
            replayed_seq: AtomicUsize::new(0),
            bytes: [const { UnsafeCell::new(0) }; BOOT_CONSOLE_BUFFER_SIZE],
        }
    }

    fn push(&self, byte: u8) {
        let seq = self.write_seq.fetch_add(1, Ordering::Relaxed);
        unsafe {
            *self.bytes[seq % BOOT_CONSOLE_BUFFER_SIZE].get() = byte;
        }
    }

    fn replay_to_runtime_console(&self) {
        #[cfg(feature = "device")]
        if let Some(console) = crate::device::console::MAIN_CONSOLE.read().as_ref() {
            let write_seq = self.write_seq.load(Ordering::Acquire);
            let oldest = write_seq.saturating_sub(BOOT_CONSOLE_BUFFER_SIZE);
            let start = self.replayed_seq.load(Ordering::Acquire).max(oldest);

            for seq in start..write_seq {
                let byte = unsafe { *self.bytes[seq % BOOT_CONSOLE_BUFFER_SIZE].get() };
                console.write_bytes(&[byte]);
            }

            self.replayed_seq.store(write_seq, Ordering::Release);
        }
    }
}

static BOOT_CONSOLE_BUFFER: BootConsoleBuffer = BootConsoleBuffer::new();

/// 切换到运行时控制台（设备初始化完成后调用）
pub fn init() {
    CONSOLE_RUNTIME.store(true, Ordering::Release);
    BOOT_CONSOLE_BUFFER.replay_to_runtime_console();
}

pub fn is_runtime() -> bool {
    CONSOLE_RUNTIME.load(Ordering::Acquire)
}

#[inline]
fn write_str_unlocked(s: &str) {
    #[cfg(feature = "device")]
    if CONSOLE_RUNTIME.load(Ordering::Acquire)
        && let Some(console) = crate::device::console::MAIN_CONSOLE.read().as_ref()
    {
        console.write_str(s);
        return;
    }

    for b in s.bytes() {
        BOOT_CONSOLE_BUFFER.push(b);
        crate::arch::ArchImpl::console_putchar(b);
    }
}

/// 无锁的单字符输出（内部使用）
#[inline]
fn putchar_unlocked(c: u8) {
    #[cfg(feature = "device")]
    if CONSOLE_RUNTIME.load(Ordering::Acquire) {
        // 运行时：使用 device console
        if let Some(console) = crate::device::console::MAIN_CONSOLE.read().as_ref() {
            console.write_bytes(&[c]);
        } else {
            // 降级到 SBI
            crate::arch::ArchImpl::console_putchar(c);
        }
        return;
    }
    // 早期或无 device 功能：使用 arch console
    BOOT_CONSOLE_BUFFER.push(c);
    crate::arch::ArchImpl::console_putchar(c);
}

/// 无锁的单字符输入（内部使用）
#[inline]
fn getchar_unlocked() -> Option<u8> {
    #[cfg(feature = "device")]
    if CONSOLE_RUNTIME.load(Ordering::Acquire) {
        // 运行时：使用 device console
        if let Some(console) = crate::device::console::MAIN_CONSOLE.read().as_ref() {
            let ch = console.read_char();
            return Some(ch as u8);
        } else {
            // 降级到 arch console
            return crate::arch::ArchImpl::console_getchar();
        }
    }
    // 早期或无 device 功能：使用 arch console
    crate::arch::ArchImpl::console_getchar()
}

fn with_console_lock_or_fallback(f: impl FnOnce()) {
    if let Some(_guard) = CONSOLE_LOCK.try_lock() {
        f();
    } else {
        f();
    }
}

/// 带锁的字符串输出（公开接口）
pub fn write_str(s: &str) {
    with_console_lock_or_fallback(|| write_str_unlocked(s));
}

/// 带锁的单字符输出（公开接口，用于兼容性）
pub fn putchar(c: u8) {
    with_console_lock_or_fallback(|| putchar_unlocked(c));
}

/// 带锁的单字符输入（公开接口）
pub fn getchar() -> Option<u8> {
    if let Some(_guard) = CONSOLE_LOCK.try_lock() {
        getchar_unlocked()
    } else {
        None
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    Stdout.write_fmt(args).unwrap();
}

/// 控制台输出结构体（实现 Write trait，供日志系统使用）
pub struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        crate::console::write_str(s);
        Ok(())
    }

    fn write_fmt(&mut self, args: fmt::Arguments) -> fmt::Result {
        // 重写 write_fmt 以确保整个格式化输出在一个锁内完成
        // 这样可以防止多个 CPU 的日志输出交错
        if let Some(_guard) = CONSOLE_LOCK.try_lock() {
            UnlockedWriter.write_fmt(args)
        } else {
            UnlockedWriter.write_fmt(args)
        }
    }
}

// 创建一个临时的 writer，它使用 write_str_unlocked（不加锁）
struct UnlockedWriter;
impl Write for UnlockedWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_str_unlocked(s);
        Ok(())
    }
}

/// Emergency print: no console lock; runtime uses MAIN_CONSOLE, otherwise boot buffer + arch mirror.
#[doc(hidden)]
pub fn emergency_print(args: core::fmt::Arguments) {
    struct EarlyWriter;
    impl core::fmt::Write for EarlyWriter {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            write_str_unlocked(s);
            Ok(())
        }
    }
    EarlyWriter.write_fmt(args).unwrap();
}

/// 打印格式化文本到控制台并写入日志缓冲区。
#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::log::print_impl(format_args!($fmt $(, $($arg)+)?))
    }
}

/// 打印格式化文本到控制台并添加换行，同时写入日志缓冲区。
#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::log::print_impl(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}
