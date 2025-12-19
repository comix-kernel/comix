//! LoongArch64 QEMU virt 平台定义

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
}

/// 获取 VirtIO 设备的 MMIO 地址和大小
/// 返回 (base_address, size)
pub fn mmio_of(device: VirtDevice) -> Option<(usize, usize)> {
    match device {
        VirtDevice::Block => Some((0x10008000, 0x1000)),
        VirtDevice::Network => Some((0x10009000, 0x1000)),
        VirtDevice::Gpu => Some((0x1000a000, 0x1000)),
        VirtDevice::Input => Some((0x1000b000, 0x1000)),
        VirtDevice::VirtPcieMmio => Some((0x20000000, 0x10000000)),  // TODO: 验证 LoongArch 地址
        VirtDevice::VirtPcieEcam => Some((0x30000000, 0x10000000)),  // TODO: 验证 LoongArch 地址
    }
}
