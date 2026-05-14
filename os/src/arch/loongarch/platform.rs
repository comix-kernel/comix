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
    Block,
    Network,
    Gpu,
    Input,
    VirtPcieMmio,
    VirtPcieEcam,
    Uart,
    Rtc,
}

/// MMIO 设备映射表
/// 格式: (设备类型, 基地址, 大小)
/// 注意：第一个条目会被测试用例使用，避免使用 UART（可能与 console 冲突）
pub const MMIO: &[(VirtDevice, usize, usize)] = &[
    (VirtDevice::Rtc, 0x10081000, 0x1000),
    (VirtDevice::Block, 0x10008000, 0x1000),
    (VirtDevice::Network, 0x10009000, 0x1000),
    (VirtDevice::Gpu, 0x1000a000, 0x1000),
    (VirtDevice::Input, 0x1000b000, 0x1000),
    (VirtDevice::Uart, 0x1fe001e0, 0x100),
    (VirtDevice::VirtPcieMmio, 0x20000000, 0x10000000),
    (VirtDevice::VirtPcieEcam, 0x30000000, 0x10000000),
];

/// 获取 VirtIO 设备的 MMIO 地址和大小
/// 返回 (base_address, size)
pub fn mmio_of(device: VirtDevice) -> Option<(usize, usize)> {
    MMIO.iter()
        .find(|(d, _, _)| *d == device)
        .map(|(_, b, s)| (*b, *s))
}

/// 初始化平台
pub fn init() {
    crate::device::serial::uart16550::driver_init();
    crate::device::bus::virtio_mmio::driver_init();
    crate::device::rtc::rtc_goldfish::driver_init();
    crate::device::device_tree::init();
    crate::device::bus::pcie::init_virtio_pci();
    crate::device::console::init();
}

/// 兼容性模块别名
pub mod virt {
    pub use super::*;
}
