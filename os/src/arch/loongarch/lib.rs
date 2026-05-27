//! LoongArch64 平台操作模块
//!
//! LoongArch 不使用 RISC-V 的 SBI，此模块通过直接 MMIO 访问
//! 提供等效功能（UART 控制台、ACPI GED 电源管理）。
//!
//! 兼容性别名 `sbi` 模块用于共享代码调用路径的过渡，
//! 待 HAL trait 覆盖这些功能后可移除。

use super::{constant::DMW0_BASE, platform::UART_BASE};

/// 通过 DMW0 映射的 UART 虚拟地址
/// DMW0: 0x8000_xxxx_xxxx_xxxx -> 物理地址 (uncached, 用于 MMIO)
const UART_VADDR: usize = UART_BASE | DMW0_BASE;

/// 输出字符到控制台
pub fn console_putchar(c: u8) {
    unsafe {
        // 等待 UART 发送缓冲区空闲 (LSR bit 5)
        let ptr = UART_VADDR as *mut u8;
        while ptr.add(5).read_volatile() & (1 << 5) == 0 {}
        ptr.write_volatile(c);
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

/// QEMU virt 平台 ACPI GED 控制寄存器基地址
/// 该地址直接指向了控制寄存器区域的起始位置
const VIRT_GED_REG_ADDR: usize = 0x100e001c;

/// 寄存器偏移 (相对于 0x100e001c)
/// 根据设备树中的 poweroff { offset = <0x00> } 和 reboot { offset = <0x02> }
const ACPI_GED_REG_SLEEP_CTL: usize = 0x00;
const ACPI_GED_REG_RESET: usize = 0x02;

/// 写入数值 (根据设备树中的 value 属性)
const ACPI_GED_VALUE_POWEROFF: u8 = 0x34; // poweroff { value = <0x34> }
const ACPI_GED_VALUE_REBOOT: u8 = 0x42; // reboot { value = <0x42> }

/// 关机实现
pub fn shutdown(_failure: bool) -> ! {
    // 映射到 LoongArch 的虚地址 (DMW0: 0x8000...)
    let base_vaddr = VIRT_GED_REG_ADDR | DMW0_BASE;

    unsafe {
        let ptr = base_vaddr as *mut u8;

        // 1. 尝试执行 Poweroff (写入 0x34 到 offset 0)
        ptr.add(ACPI_GED_REG_SLEEP_CTL)
            .write_volatile(ACPI_GED_VALUE_POWEROFF);

        // 2. 如果关机失败，尝试执行 Reboot (写入 0x42 到 offset 2)
        // 注意：根据你的 DTS，reboot 的 value 是 0x42，offset 是 0x02
        ptr.add(ACPI_GED_REG_RESET)
            .write_volatile(ACPI_GED_VALUE_REBOOT);
    }

    // 如果硬件没有响应，进入死循环
    loop {
        unsafe {
            // LoongArch 的休眠指令
            core::arch::asm!("idle 0");
        }
    }
}

/// 重启实现
pub fn restart() -> ! {
    let base_vaddr = VIRT_GED_REG_ADDR | DMW0_BASE;

    unsafe {
        let ptr = base_vaddr as *mut u8;
        ptr.add(ACPI_GED_REG_RESET)
            .write_volatile(ACPI_GED_VALUE_REBOOT);
    }

    loop {
        unsafe {
            core::arch::asm!("idle 0");
        }
    }
}
