//! LoongArch64 SBI 兼容模块（存根）
//!
//! LoongArch 不使用 SBI，此模块仅用于兼容 RISC-V 代码

// TODO: 请重构代码使得Comix不再需要为LA实现SBI的占位符

use super::super::platform::virt::UART_BASE;

/// 通过 DMW0 映射的 UART 虚拟地址
/// DMW0: 0x8000_xxxx_xxxx_xxxx -> 物理地址 (uncached, 用于 MMIO)
const UART_VADDR: usize = UART_BASE | 0x8000_0000_0000_0000;

/// 输出字符到控制台
pub fn console_putchar(c: usize) {
    unsafe {
        // 等待 UART 发送缓冲区空闲 (LSR bit 5)
        let ptr = UART_VADDR as *mut u8;
        while ptr.add(5).read_volatile() & (1 << 5) == 0 {}
        ptr.write_volatile(c as u8);
    }
}

/// 从控制台读取字符
pub fn console_getchar() -> usize {
    unsafe {
        let ptr = UART_VADDR as *mut u8;
        // 检查接收缓冲区是否有数据 (LSR bit 0)
        if ptr.add(5).read_volatile() & 1 == 0 {
            usize::MAX // 无数据
        } else {
            ptr.read_volatile() as usize
        }
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
