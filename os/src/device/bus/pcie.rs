//! PCIe 模块

use crate::{
    config::{VirtDevice, mmio_of},
    device::{block::virtio_blk, device_tree::FDT, net::virtio_net},
    kernel::current_memory_space,
    mm::{
        address::{ConvertablePaddr, Paddr, UsizeConvert},
        page_table::PagingError,
    },
    pr_info, pr_warn,
};
use core::ptr::{read_volatile, write_volatile};
use virtio_drivers::{
    transport::pci::bus::{BarInfo, Cam, Command, MemoryBarType, MmioCam, PciRoot},
    transport::{
        DeviceType,
        pci::{PciTransport, virtio_device_type},
    },
};

use crate::device::virtio_hal::VirtIOHal;

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
        // fdt 0.1.5 没有公开 parent 访问，暂时使用通用默认值
        let parent_addr_cells = 2usize;
        let parent_size_cells = 2usize;

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
        let (ecam_base, ecam_size, ecam_from_fdt) = if let Some(prop) = node.property("reg") {
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
            (addr, size, true)
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

                if is_mem && size != 0 {
                    match mmio {
                        Some((_, best_size)) if best_size >= size => {}
                        _ => {
                            mmio = Some((parent_addr, size));
                        }
                    }
                }

                off += entry_bytes;
            }

            mmio.unwrap_or_else(|| mmio_of(VirtDevice::VirtPcieMmio).unwrap())
        } else {
            mmio_of(VirtDevice::VirtPcieMmio)?
        };

        let mut ecam_base = ecam_base;
        let mut ecam_size = ecam_size;
        let mut ecam_from_fdt = ecam_from_fdt;
        let mut mmio_base = mmio_base;
        let mut mmio_size = mmio_size;

        if ecam_size == 0 {
            if let Some((def_base, def_size)) = mmio_of(VirtDevice::VirtPcieEcam) {
                ecam_base = def_base;
                ecam_size = def_size;
                ecam_from_fdt = false;
                pr_warn!("[PCIe] FDT ECAM missing, using platform defaults");
            }
        }

        if mmio_size < 0x100000 {
            if let Some((def_base, def_size)) = mmio_of(VirtDevice::VirtPcieMmio) {
                mmio_base = def_base;
                mmio_size = def_size;
                pr_warn!("[PCIe] FDT PCIe ranges too small, overriding MMIO window");
            }
        }

        let ecam_end = ecam_base.saturating_add(ecam_size);
        let mmio_end = mmio_base.saturating_add(mmio_size);
        let overlap =
            ecam_size != 0 && mmio_size != 0 && ecam_base < mmio_end && mmio_base < ecam_end;
        if overlap {
            if ecam_from_fdt {
                if let Some((def_base, def_size)) = mmio_of(VirtDevice::VirtPcieMmio) {
                    mmio_base = def_base;
                    mmio_size = def_size;
                    pr_warn!("[PCIe] FDT ECAM overlaps MMIO, overriding MMIO window");
                }
            } else if let Some((def_base, def_size)) = mmio_of(VirtDevice::VirtPcieEcam) {
                ecam_base = def_base;
                ecam_size = def_size;
                pr_warn!("[PCIe] FDT ECAM overlaps MMIO, using platform defaults");
            }
        }

        pr_info!(
            "[PCIe] FDT ECAM base={:#x} size={:#x}, MMIO base={:#x} size={:#x}",
            ecam_base,
            ecam_size,
            mmio_base,
            mmio_size
        );

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

/// 初始化 PCIe 并枚举 VirtIO PCI 设备
pub fn init_virtio_pci() {
    let host =
        if let Some(host) = PcieHost::from_fdt().or_else(|| PcieHost::from_platform_defaults()) {
            host
        } else {
            pr_info!("[PCIe] no host found from platform defaults");
            return;
        };

    let ecam_vaddr = match current_memory_space()
        .lock()
        .map_mmio(Paddr::from_usize(host.ecam_paddr), host.ecam_size)
    {
        Ok(vaddr) => vaddr.as_usize(),
        Err(PagingError::AlreadyMapped) => crate::arch::mm::paddr_to_vaddr(host.ecam_paddr),
        Err(e) => {
            pr_warn!("[PCIe] failed to map ECAM: {:?}", e);
            crate::arch::mm::paddr_to_vaddr(host.ecam_paddr)
        }
    };

    match current_memory_space()
        .lock()
        .map_mmio(Paddr::from_usize(host.mmio_base), host.mmio_size)
    {
        Ok(_) | Err(PagingError::AlreadyMapped) => {}
        Err(e) => {
            pr_warn!("[PCIe] failed to map PCIe MMIO window: {:?}", e);
        }
    }

    let cam = unsafe { MmioCam::new(ecam_vaddr as *mut u8, Cam::Ecam) };
    let mut root = PciRoot::new(cam);
    let mut next_mmio = host.mmio_base;
    let mmio_end = host.mmio_base.saturating_add(host.mmio_size);

    for bus in host.bus_start..=host.bus_end {
        for (df, info) in root.enumerate_bus(bus) {
            let dev_type = match virtio_device_type(&info) {
                Some(t) => t,
                None => continue,
            };

            let (_status, command) = root.get_status_command(df);
            let new_command = command | Command::MEMORY_SPACE | Command::BUS_MASTER;
            if new_command != command {
                root.set_command(df, new_command);
            }

            allocate_bars(&mut root, df, &mut next_mmio, mmio_end);

            let transport = match PciTransport::new::<VirtIOHal, _>(&mut root, df) {
                Ok(t) => t,
                Err(e) => {
                    pr_warn!(
                        "[PCIe] failed to init virtio-pci {} ({:?}): {:?}",
                        df,
                        info,
                        e
                    );
                    continue;
                }
            };

            match dev_type {
                DeviceType::Block => virtio_blk::init_pci(transport),
                DeviceType::Network => virtio_net::init_pci(transport),
                _ => {
                    pr_info!("[PCIe] virtio device {:?} not wired yet", dev_type);
                }
            }
        }
    }
}

fn allocate_bars(
    root: &mut PciRoot<MmioCam<'_>>,
    df: virtio_drivers::transport::pci::bus::DeviceFunction,
    next_mmio: &mut usize,
    mmio_end: usize,
) {
    let bars = match root.bars(df) {
        Ok(bars) => bars,
        Err(e) => {
            pr_warn!("[PCIe] failed to read BARs for {}: {:?}", df, e);
            return;
        }
    };

    let mut bar_index = 0u8;
    while bar_index < 6 {
        let info = bars[usize::from(bar_index)].clone();
        let step = info
            .as_ref()
            .map_or(1, |b| if b.takes_two_entries() { 2 } else { 1 });

        if let Some(BarInfo::Memory {
            address_type,
            address,
            size,
            ..
        }) = info
        {
            if size == 0 || address != 0 {
                bar_index += step;
                continue;
            }
            if address_type == MemoryBarType::Below1MiB {
                pr_warn!(
                    "[PCIe] BAR{} requires below-1MiB window, skipping",
                    bar_index
                );
                bar_index += step;
                continue;
            }
            let size_usize = size as usize;
            let align = size_usize.max(0x1000);
            let base = align_up(*next_mmio, align);
            if base.saturating_add(size_usize) > mmio_end {
                pr_warn!(
                    "[PCIe] MMIO window exhausted for BAR{} (need {:#x} bytes)",
                    bar_index,
                    size
                );
                bar_index += step;
                continue;
            }
            match address_type {
                MemoryBarType::Width64 => root.set_bar_64(df, bar_index, base as u64),
                _ => root.set_bar_32(df, bar_index, base as u32),
            }
            *next_mmio = base.saturating_add(size_usize);
        }

        bar_index += step;
    }
}

#[inline]
fn align_up(value: usize, align: usize) -> usize {
    if align == 0 {
        value
    } else {
        (value + align - 1) & !(align - 1)
    }
}
