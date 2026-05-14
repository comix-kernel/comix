pub const MEMORY_END: usize = 0x8800_0000;
pub const VIRT_CPUS_MAX: usize = 4;

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

pub fn mmio_of(_dev: VirtDevice) -> Option<(usize, usize)> {
    None
}

pub fn init() {}
