//! LoongArch64 平台模块

mod loongarch_virt;

pub use loongarch_virt::*;

/// virt 平台别名（用于兼容 RISC-V 代码）
pub mod virt {
    pub use super::loongarch_virt::*;
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
