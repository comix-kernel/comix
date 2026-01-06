//! 设备树模块

use crate::{
    device::{CMDLINE, irq::IntcDriver},
    kernel::{CLOCK_FREQ, NUM_CPU},
    mm::address::{ConvertablePaddr, Paddr, UsizeConvert},
    pr_info, pr_warn,
};
use alloc::{collections::btree_map::BTreeMap, string::String, sync::Arc};
use fdt::{Fdt, node::FdtNode};
use spin::RwLock;
/// 指向设备树的指针，在启动时由引导程序设置
#[unsafe(no_mangle)]
pub static mut DTP: usize = 0x114514; // 占位地址，实际由引导程序设置

lazy_static::lazy_static! {
    /// 设备树
    /// 通过 DTP 指针解析得到
    /// XXX: 是否需要这个?
    pub static ref FDT: Option<Fdt<'static>> = {
        unsafe {
            let addr = Paddr::to_vaddr(&Paddr::from_usize(DTP));
            fdt::Fdt::from_ptr(addr.as_usize() as *mut u8).ok()
        }
    };

    /// Compatible 字符串到探测函数的映射表
    /// 键为设备的 compatible 字符串，值为对应的探测函数
    /// 用于在设备树中查找和初始化设备
    pub static ref DEVICE_TREE_REGISTRY: RwLock<BTreeMap<&'static str, fn(&FdtNode)>> =
        RwLock::new(BTreeMap::new());

    /// 设备树中断控制器映射表
    /// 键为中断控制器的 phandle，值为对应的中断控制器驱动程序
    /// 用于在设备树中查找和管理中断控制器
    pub static ref DEVICE_TREE_INTC: RwLock<BTreeMap<u32, Arc<dyn IntcDriver>>> =
        RwLock::new(BTreeMap::new());
}

/// 初始化设备树
pub fn init() {
    let fdt = match FDT.as_ref() {
        Some(fdt) => fdt,
        None => {
            pr_warn!("[Device] Failed to parse device tree, skipping init");
            return;
        }
    };

    let model = fdt
        .root()
        .property("model")
        .and_then(|p| p.value.split(|b| *b == 0).next())
        .and_then(|s| core::str::from_utf8(s).ok())
        .unwrap_or("unknown");
    pr_info!("[Device] devicetree of {} is initialized", model);

    let cpus = fdt.cpus().count();
    // SAFETY: 这里是在单核初始化阶段设置 CPU 数量
    unsafe { NUM_CPU = cpus };
    pr_info!("[Device] now has {} CPU(s)", cpus);

    if let Some(cpu) = fdt.cpus().next() {
        let timebase = cpu
            .property("timebase-frequency")
            .or_else(|| cpu.property("clock-frequency"))
            .and_then(|p| match p.value.len() {
                4 => Some(u32::from_be_bytes(p.value.try_into().ok()?) as usize),
                8 => Some(u64::from_be_bytes(p.value.try_into().ok()?) as usize),
                _ => None,
            });
        if let Some(freq) = timebase {
            unsafe {
                CLOCK_FREQ = freq;
            }
            pr_info!("[Device] CLOCK_FREQ set to {} Hz", unsafe { CLOCK_FREQ });
        } else {
            pr_warn!("[Device] No timebase-frequency in DTB, keeping default");
        }
    } else {
        pr_warn!("[Device] No CPU found in device tree");
    }

    fdt.memory().regions().for_each(|region| {
        pr_info!(
            "[Device] Memory Region: Start = {:#X}, Size = {:#X}",
            region.starting_address as usize,
            region.size.unwrap() as usize
        );
    });

    if let Some(bootargs) = fdt.chosen().bootargs() {
        if !bootargs.is_empty() {
            pr_info!("Kernel cmdline: {}", bootargs);
            *CMDLINE.write() = String::from(bootargs);
        }
    }

    // 首先初始化中断控制器
    walk_dt(fdt, true);
    walk_dt(fdt, false);
}

/// 遍历设备树，查找并初始化 virtio 设备
/// # 参数
/// * `fdt` - 设备树对象
fn walk_dt(fdt: &Fdt, intc_only: bool) {
    for node in fdt.all_nodes() {
        if let Some(compatible) = node.compatible() {
            if node.property("interrupt-controller").is_some() == intc_only {
                pr_info!("[Device] Found device: {}", node.name);
                let registry = DEVICE_TREE_REGISTRY.read();
                for c in compatible.all() {
                    if let Some(f) = registry.get(c) {
                        f(&node);
                    }
                }
            }
        }
    }
}

/// 返回 DRAM 的起始物理地址与总大小（合并所有 memory.regions）
/// # 返回值
/// * `Option<(usize, usize)>` - 返回起始地址和大小的元组，如果没有有效的内存区域则返回 None
pub fn dram_info() -> Option<(usize, usize)> {
    let fdt = FDT.as_ref()?;
    let mut start = usize::MAX;
    let mut end = 0usize;

    for region in fdt.memory().regions() {
        let s = region.starting_address as usize;
        let size = region.size.unwrap_or(0) as usize;
        let e = s.saturating_add(size);
        if size == 0 {
            continue;
        }
        if s < start {
            start = s;
        }
        if e > end {
            end = e;
        }
    }

    if start < end {
        Some((start, end - start))
    } else {
        None
    }
}
