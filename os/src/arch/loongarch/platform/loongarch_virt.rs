//! LoongArch64 QEMU virt 平台定义
//!
//! 参考 QEMU LoongArch virt 机器定义

/// UART 基地址
pub const UART_BASE: usize = 0x1fe001e0;

/// 内存起始地址
pub const MEMORY_START: usize = 0x0;

/// 内存结束地址 (假设 128MB)
pub const MEMORY_END: usize = 0x8000000;

/// 设备基地址
pub const DEVICE_BASE: usize = 0x10000000;

/// 设备结束地址
pub const DEVICE_END: usize = 0x30000000;

/// VirtIO 设备类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtDevice {
    /// VirtIO 块设备
    Block,
    /// VirtIO 网络设备
    Network,
    /// VirtIO GPU
    Gpu,
    /// VirtIO 输入设备
    Input,
    /// VirtIO PCIe MMIO
    VirtPcieMmio,
    /// VirtIO PCIe ECAM
    VirtPcieEcam,
    /// UART
    Uart,
    /// RTC
    Rtc,
}

/// MMIO 设备映射表
/// 格式: (设备类型, 基地址, 大小)
/// 注意：第一个条目会被测试用例使用，避免使用 UART（可能与 console 冲突）
pub const MMIO: &[(VirtDevice, usize, usize)] = &[
    // LoongArch QEMU virt 平台 MMIO 布局
    // RTC 放在第一位，用于测试（不容易冲突）
    (VirtDevice::Rtc, 0x10081000, 0x1000),     // RTC (Goldfish)
    (VirtDevice::Block, 0x10008000, 0x1000),   // VirtIO Block
    (VirtDevice::Network, 0x10009000, 0x1000), // VirtIO Network
    (VirtDevice::Gpu, 0x1000a000, 0x1000),     // VirtIO GPU
    (VirtDevice::Input, 0x1000b000, 0x1000),   // VirtIO Input
    (VirtDevice::Uart, 0x1fe001e0, 0x100),     // UART (放在后面，避免测试冲突)
    (VirtDevice::VirtPcieMmio, 0x20000000, 0x10000000), // PCIe MMIO
    (VirtDevice::VirtPcieEcam, 0x30000000, 0x10000000), // PCIe ECAM
];

/// 获取 VirtIO 设备的 MMIO 地址和大小
/// 返回 (base_address, size)
pub fn mmio_of(device: VirtDevice) -> Option<(usize, usize)> {
    MMIO.iter()
        .find(|(d, _, _)| *d == device)
        .map(|(_, b, s)| (*b, *s))
}
