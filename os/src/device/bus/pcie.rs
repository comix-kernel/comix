//! PCIe 模块

use crate::{
    config::{VirtDevice, mmio_of},
    device::device_tree::FDT,
    mm::address::{ConvertablePaddr, Paddr, UsizeConvert},
    pr_info,
};
use core::ptr::{read_volatile, write_volatile};

/// PCIe 设备结构体
pub struct PciDevice {}

/// PCIe 驱动结构体
pub struct PciDriver {}

/// PCIe Host 控制器结构体
#[derive(Clone, Copy)]
pub struct PcieHost {
    /// ECAM 物理地址
    ecam_paddr: usize,
    /// ECAM 虚拟地址
    ecam_vaddr: usize,
    /// ECAM 大小
    ecam_size: usize,
    /// MMIO 基地址
    mmio_base: usize,
    /// MMIO 大小
    mmio_size: usize,
    /// 总线起始号
    bus_start: u8,
    /// 总线结束号
    bus_end: u8,
}

impl PcieHost {
    /// 从设备树解析
    pub fn from_fdt() -> Option<Self> {
        let fdt = &*FDT;

        // 找到兼容 pci-host-ecam-generic 的节点
        let mut target = None;
        for node in fdt.all_nodes() {
            if let Some(prop) = node.property("compatible") {
                let bytes = prop.value;
                let mut start = 0;
                while start < bytes.len() {
                    let mut end = start;
                    while end < bytes.len() && bytes[end] != 0 {
                        end += 1;
                    }
                    if end > start && &bytes[start..end] == b"pci-host-ecam-generic" {
                        target = Some(node);
                        break;
                    }
                    start = end + 1;
                }
                if target.is_some() {
                    break;
                }
            }
        }
        let node = target?;

        // 父节点（通常是 /soc 或根）决定 reg/ranges 中 parent addr/size cells
        let parent = node.interrupt_parent()?;

        let child_addr_cells = node
            .property("#address-cells")
            .and_then(|p| {
                if p.value.len() >= 4 {
                    Some(u32::from_be_bytes(p.value[0..4].try_into().unwrap()) as usize)
                } else {
                    None
                }
            })
            .unwrap_or(3); // PCI 默认 3

        let child_size_cells = node
            .property("#size-cells")
            .and_then(|p| {
                if p.value.len() >= 4 {
                    Some(u32::from_be_bytes(p.value[0..4].try_into().unwrap()) as usize)
                } else {
                    None
                }
            })
            .unwrap_or(2);

        let parent_addr_cells = parent
            .property("#address-cells")
            .and_then(|p| {
                if p.value.len() >= 4 {
                    Some(u32::from_be_bytes(p.value[0..4].try_into().unwrap()) as usize)
                } else {
                    None
                }
            })
            .unwrap_or(2);

        let parent_size_cells = parent
            .property("#size-cells")
            .and_then(|p| {
                if p.value.len() >= 4 {
                    Some(u32::from_be_bytes(p.value[0..4].try_into().unwrap()) as usize)
                } else {
                    None
                }
            })
            .unwrap_or(2);

        // bus-range（两个 32-bit）
        let (bus_start, bus_end) = if let Some(prop) = node.property("bus-range") {
            if prop.value.len() >= 8 {
                let b0 = u32::from_be_bytes(prop.value[0..4].try_into().unwrap());
                let b1 = u32::from_be_bytes(prop.value[4..8].try_into().unwrap());
                (b0 as u8, b1 as u8)
            } else {
                (0, 255)
            }
        } else {
            (0, 255)
        };

        // reg: 假设 parent_addr_cells=2 parent_size_cells=2 => 4 cells = 16 字节
        let (ecam_base, ecam_size) = if let Some(prop) = node.property("reg") {
            let cells = parent_addr_cells + parent_size_cells;
            let need = cells * 4;
            if prop.value.len() < need {
                return None;
            }
            let mut idx = 0;
            let mut addr = 0usize;
            for _ in 0..parent_addr_cells {
                addr = (addr << 32)
                    | u32::from_be_bytes(prop.value[idx..idx + 4].try_into().unwrap()) as usize;
                idx += 4;
            }
            let mut size = 0usize;
            for _ in 0..parent_size_cells {
                size = (size << 32)
                    | u32::from_be_bytes(prop.value[idx..idx + 4].try_into().unwrap()) as usize;
                idx += 4;
            }
            (addr, size)
        } else {
            return None;
        };

        // 解析 ranges：迭代条目，选取第一个 memory 资源作为 mmio window
        let (mmio_base, mmio_size) = if let Some(prop) = node.property("ranges") {
            let entry_cells = child_addr_cells + parent_addr_cells + parent_size_cells;
            let entry_bytes = entry_cells * 4;
            let mut mmio: Option<(usize, usize)> = None;
            let mut off = 0;
            while off + entry_bytes <= prop.value.len() {
                let mut idx = off;

                // child 地址（第一 cell 为 flags）
                let mut child_cells_vals: [u32; 8] = [0; 8];
                for c in 0..child_addr_cells {
                    child_cells_vals[c] =
                        u32::from_be_bytes(prop.value[idx..idx + 4].try_into().unwrap());
                    idx += 4;
                }
                let flags = child_cells_vals[0];

                // 仅处理 memory 空间 (flags & 0x0300_0000 == 0x0100_0000 或 0x0200_0000)
                let space_code = flags & 0x0300_0000;
                let is_mem = space_code == 0x0100_0000 || space_code == 0x0200_0000;
                // 组合 child 基址（后两个 cells）
                let mut child_addr = 0usize;
                for c in 1..child_addr_cells {
                    child_addr = (child_addr << 32) | child_cells_vals[c] as usize;
                }

                // parent 地址
                let mut parent_addr = 0usize;
                for _ in 0..parent_addr_cells {
                    parent_addr = (parent_addr << 32)
                        | u32::from_be_bytes(prop.value[idx..idx + 4].try_into().unwrap()) as usize;
                    idx += 4;
                }

                // size
                let mut size = 0usize;
                for _ in 0..parent_size_cells {
                    size = (size << 32)
                        | u32::from_be_bytes(prop.value[idx..idx + 4].try_into().unwrap()) as usize;
                    idx += 4;
                }

                if is_mem && size != 0 && mmio.is_none() {
                    mmio = Some((parent_addr, size));
                }

                off += entry_bytes;
            }

            mmio.unwrap_or_else(|| mmio_of(VirtDevice::VirtPcieMmio).unwrap())
        } else {
            mmio_of(VirtDevice::VirtPcieMmio)?
        };

        let ecam_vaddr = Paddr::to_vaddr(&Paddr::from_usize(ecam_base)).as_usize();

        Some(Self {
            ecam_paddr: ecam_base,
            ecam_vaddr,
            ecam_size,
            mmio_base,
            mmio_size,
            bus_start,
            bus_end,
        })
    }

    /// 从平台默认配置解析
    pub fn from_platform_defaults() -> Option<Self> {
        let (ecam_base, ecam_size) = mmio_of(VirtDevice::VirtPcieEcam)?;
        let (mmio_base, mmio_size) = mmio_of(VirtDevice::VirtPcieMmio)?;
        let ecam_vaddr = Paddr::to_vaddr(&Paddr::from_usize(ecam_base)).as_usize();

        Some(Self {
            ecam_paddr: ecam_base,
            ecam_vaddr,
            ecam_size,
            mmio_base,
            mmio_size,
            bus_start: 0,
            bus_end: 255, // 后续可由 FDT 的 bus-range 收缩
        })
    }

    /// 计算配置空间地址
    #[inline]
    fn cfg_addr(&self, bus: u8, dev: u8, func: u8, offset: u16) -> *mut u32 {
        // ECAM: base + (bus<<20) + (dev<<15) + (func<<12) + offset
        let off = ((bus as usize) << 20)
            | ((dev as usize) << 15)
            | ((func as usize) << 12)
            | ((offset as usize) & 0xfff);
        (self.ecam_vaddr + off) as *mut u32
    }

    /// 读取 PCIe 配置空间
    pub unsafe fn cfg_read32(&self, bus: u8, dev: u8, func: u8, offset: u16) -> u32 {
        unsafe { read_volatile(self.cfg_addr(bus, dev, func, offset)) }
    }

    /// 写入 PCIe 配置空间
    pub unsafe fn cfg_write32(&self, bus: u8, dev: u8, func: u8, offset: u16, val: u32) {
        unsafe {
            write_volatile(self.cfg_addr(bus, dev, func, offset), val);
        }
    }

    /// 枚举 PCIe 设备并打印信息
    pub fn enumerate(&self) {
        pr_info!(
            "[PCIe] ECAM @ {:#x} (size {:#x}), MMIO @ {:#x} (size {:#x})",
            self.ecam_paddr,
            self.ecam_size,
            self.mmio_base,
            self.mmio_size
        );

        for bus in self.bus_start..=self.bus_end {
            for dev in 0u8..32 {
                // 先探测 function 0 是否存在（Vendor ID 全 1 表示不存在）
                let vend = unsafe { self.cfg_read32(bus, dev, 0, 0x00) } as u16;
                if vend == 0xffff {
                    continue;
                }
                // 读取 func0 的设备信息
                let did = (unsafe { self.cfg_read32(bus, dev, 0, 0x00) } >> 16) as u16;
                let hdr = (unsafe { self.cfg_read32(bus, dev, 0, 0x0C) } >> 16) as u8; // header type
                let multi_func = (hdr & 0x80) != 0;

                pr_info!(
                    "[PCIe] bus {:02x} dev {:02x} fn 00: vendor={:04x} device={:04x} hdr={:02x}",
                    bus,
                    dev,
                    vend,
                    did,
                    hdr & 0x7f
                );

                // 如为多功能设备，继续探测 fn1..7
                if multi_func {
                    for func in 1u8..8 {
                        let vend = unsafe { self.cfg_read32(bus, dev, func, 0x00) } as u16;
                        if vend == 0xffff {
                            continue;
                        }
                        let did = (unsafe { self.cfg_read32(bus, dev, func, 0x00) } >> 16) as u16;
                        let hdr = (unsafe { self.cfg_read32(bus, dev, func, 0x0C) } >> 16) as u8;
                        pr_info!(
                            "[PCIe] bus {:02x} dev {:02x} fn {:02x}: vendor={:04x} device={:04x} hdr={:02x}",
                            bus,
                            dev,
                            func,
                            vend,
                            did,
                            hdr & 0x7f
                        );
                    }
                }
            }
        }
    }
}

/// 初始化并枚举 PCIe 设备
pub fn init_and_enumerate() {
    if let Some(host) = PcieHost::from_fdt().or_else(|| PcieHost::from_platform_defaults()) {
        host.enumerate();
    } else {
        pr_info!("[PCIe] no host found from platform defaults");
    }
}
