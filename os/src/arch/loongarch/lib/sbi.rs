//! LoongArch64 SBI 兼容模块（存根）
//!
//! LoongArch 不使用 SBI，此模块仅用于兼容 RISC-V 代码

// TODO: 请重构代码使得Comix不再需要为LA实现SBI的占位符

use super::super::platform::virt::UART_BASE;

/// 通过 DMW0 映射的 UART 虚拟地址
/// DMW0: 0x8000_xxxx_xxxx_xxxx -> 物理地址 (uncached, 用于 MMIO)
const UART_VADDR: usize = UART_BASE | 0x8000_0000_0000_0000;

/// QEMU virt 平台 ACPI GED 地址定义
/// 参考 QEMU include/hw/loongarch/virt.h 和 include/hw/acpi/generic_event_device.h
///
/// VIRT_GED_EVT_ADDR  = 0x100e0000
/// VIRT_GED_MEM_ADDR  = VIRT_GED_EVT_ADDR + 0x4 = 0x100e0004
/// VIRT_GED_REG_ADDR  = VIRT_GED_MEM_ADDR + 0x14 = 0x100e0018
///
/// ACPI GED 寄存器偏移:
/// - SLEEP_CTL = 0x00
/// - SLEEP_STS = 0x01  
/// - RESET     = 0x02
const VIRT_GED_REG_ADDR: usize = 0x100e0018;
const ACPI_GED_REG_SLEEP_CTL: usize = 0x00;
const ACPI_GED_REG_RESET: usize = 0x02;

/// ACPI 睡眠控制寄存器位定义 (ACPI 5.0 Chapter 4.8.3.7)
/// SLP_TYPx: bits [4:2] - 睡眠类型
/// SLP_EN:   bit 5      - 睡眠使能
const ACPI_GED_SLP_TYP_POS: u8 = 2; // SLP_TYP 位偏移
const ACPI_GED_SLP_TYP_S5: u8 = 0x05; // S5 = 关机状态
const ACPI_GED_SLP_EN: u8 = 0x20; // 睡眠使能位

/// 复位寄存器值
const ACPI_GED_RESET_VALUE: u8 = 0x42;

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
///
/// 使用 QEMU virt 平台的 ACPI GED SLEEP_CTL 寄存器触发 S5 关机
/// 参考 ACPI 5.0 规范和 QEMU hw/acpi/generic_event_device.c
pub fn shutdown(_failure: bool) -> ! {
    // 计算 SLEEP_CTL 寄存器的虚拟地址（通过 DMW0 映射）
    let sleep_ctl_addr = (VIRT_GED_REG_ADDR + ACPI_GED_REG_SLEEP_CTL) | 0x8000_0000_0000_0000;

    // 构造 SLEEP_CTL 寄存器值: SLP_TYP=5 (S5), SLP_EN=1
    let sleep_value: u8 = (ACPI_GED_SLP_TYP_S5 << ACPI_GED_SLP_TYP_POS) | ACPI_GED_SLP_EN;

    unsafe {
        let sleep_ctl = sleep_ctl_addr as *mut u8;
        sleep_ctl.write_volatile(sleep_value);
    }

    // 如果 SLEEP_CTL 关机失败，尝试复位
    let reset_addr = (VIRT_GED_REG_ADDR + ACPI_GED_REG_RESET) | 0x8000_0000_0000_0000;
    unsafe {
        let reset_reg = reset_addr as *mut u8;
        reset_reg.write_volatile(ACPI_GED_RESET_VALUE);
    }

    // 如果以上都失败，无限循环
    loop {
        unsafe {
            core::arch::asm!("idle 0");
        }
    }
}
