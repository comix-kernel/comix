//! LoongArch64 SBI 兼容模块（存根）
//!
//! LoongArch 不使用 SBI，此模块仅用于兼容 RISC-V 代码

/// 输出字符到控制台
pub fn console_putchar(c: usize) {
    unsafe {
        let uart_base = 0x1fe001e0usize;
        (uart_base as *mut u8).write_volatile(c as u8);
    }
}

/// 从控制台读取字符
pub fn console_getchar() -> usize {
    unsafe {
        let uart_base = 0x1fe001e0usize;
        (uart_base as *const u8).read_volatile() as usize
    }
}

/// 设置定时器
pub fn set_timer(_timer: usize) {
    // TODO: 实现 LoongArch 定时器设置
}

/// 关机
pub fn shutdown(_failure: bool) -> ! {
    loop {
        unsafe { core::arch::asm!("idle 0") };
    }
}
