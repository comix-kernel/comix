#![allow(dead_code)]
pub const CLOCK_FREQ: usize = 12500000;
pub const MEMORY_END: usize = 0x8800_0000;

/// MMIO[i] = (mmio_base, mmio_size)
pub const MMIO: &[(usize, usize)] = &[
    (0x0010_0000, 0x00_2000), // VIRT_TEST/RTC  in virt machine
    (0x1000_0000, 0x00_1000), // UART16550
    (0x1000_1000, 0x00_1000), // Virtio Block in virt machine
    (0x1000_2000, 0x00_1000), // Virtio Network in virt machine
    (0x0C00_0000, 0x40_0000), // PLIC
];
