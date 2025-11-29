//! RISC-V Virt 平台相关

use crate::device::{bus, console, device_tree, irq, rtc, serial};

/// 初始化 Virt 平台相关设备
pub fn init() {
    serial::uart16550::driver_init();
    bus::virtio_mmio::driver_init();
    irq::plic::driver_init();
    rtc::rtc_goldfish::driver_init();
    device_tree::init();
    console::init();
}

pub const MEMORY_END: usize = 0x8800_0000;

pub const VIRT_CPUS_MAX: usize = 4;
pub const PLIC_MIN_SIZE: usize = 0x4000;
pub const PLIC_MAX_SIZE: usize = 0x10_00000; // 16 MB
pub const VIRT_PLIC_SIZE: usize = PLIC_MAX_SIZE;

pub const APLIC_SIZE: usize = 0x1_00000; // 1 MB per domain?
pub const VIRT_IMSIC_MAX_SIZE: usize = 0x40_00000;

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VirtDevice {
    VirtDebug,
    VirtMrom,
    VirtTest,
    VirtRtc,
    VirtClint,
    VirtAclintSswi,
    VirtPlic,
    VirtAplicM,
    VirtAplicS,
    VirtUart0,
    VirtVirtio,
    VirtFwCfg,
    VirtImsicM,
    VirtImsicS,
    VirtFlash,
    VirtPciePio,
    VirtIommuSys,
    VirtPlatformBus,
    VirtPcieEcam,
    VirtPcieMmio,
    VirtDram,
}

/// 可扩展尺寸计算（如后续按 CPU 数量调整）
const fn plic_size(_cpus_x2: usize) -> usize {
    VIRT_PLIC_SIZE
}

const fn aplic_size(_cpus: usize) -> usize {
    APLIC_SIZE
}

pub const MMIO: &[(VirtDevice, usize, usize)] = &[
    (VirtDevice::VirtDebug, 0x0000_0000, 0x100),
    (VirtDevice::VirtMrom, 0x0000_1000, 0xf000),
    (VirtDevice::VirtTest, 0x0010_0000, 0x1000),
    (VirtDevice::VirtRtc, 0x0010_1000, 0x1000),
    (VirtDevice::VirtClint, 0x0200_0000, 0x10000),
    (VirtDevice::VirtAclintSswi, 0x02F0_0000, 0x4000),
    (VirtDevice::VirtPciePio, 0x0300_0000, 0x10000),
    (VirtDevice::VirtIommuSys, 0x0301_0000, 0x1000),
    (VirtDevice::VirtPlatformBus, 0x0400_0000, 0x0200_0000),
    (
        VirtDevice::VirtPlic,
        0x0C00_0000,
        plic_size(VIRT_CPUS_MAX * 2),
    ),
    (
        VirtDevice::VirtAplicM,
        0x0D00_0000,
        aplic_size(VIRT_CPUS_MAX),
    ), // XXX: 有重叠？QEMU源码如此
    (
        VirtDevice::VirtAplicS,
        0x0E00_0000,
        aplic_size(VIRT_CPUS_MAX),
    ),
    (VirtDevice::VirtUart0, 0x1000_0000, 0x100),
    (VirtDevice::VirtVirtio, 0x1000_1000, 0x1000),
    (VirtDevice::VirtFwCfg, 0x1010_0000, 0x18),
    (VirtDevice::VirtFlash, 0x2000_0000, 0x0400_0000),
    (VirtDevice::VirtImsicM, 0x2400_0000, VIRT_IMSIC_MAX_SIZE),
    (VirtDevice::VirtImsicS, 0x2800_0000, VIRT_IMSIC_MAX_SIZE),
    (VirtDevice::VirtPcieEcam, 0x3000_0000, 0x1000_0000),
    (VirtDevice::VirtPcieMmio, 0x4000_0000, 0x4000_0000),
    (VirtDevice::VirtDram, 0x8000_0000, 0), // size 0 表示"由内存探测或外部传入"
];

/// 查找设备的 (base, size)
pub fn mmio_of(dev: VirtDevice) -> Option<(usize, usize)> {
    MMIO.iter()
        .find(|(d, _, _)| *d as u32 == dev as u32)
        .map(|(_, b, s)| (*b, *s))
}
