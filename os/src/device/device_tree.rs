//! 设备树模块
use core::ptr::NonNull;

use crate::{
    kernel::current_memory_space,
    mm::address::{ConvertablePaddr, Paddr, UsizeConvert},
    pr_info, pr_warn, println,
};
use fdt::{Fdt, node::FdtNode, standard_nodes::Compatible};
use virtio_drivers::transport::{
    Transport,
    mmio::{MmioTransport, VirtIOHeader},
};

/// 指向设备树的指针，在启动时由引导程序设置
#[unsafe(no_mangle)]
pub static mut DTP: usize = 0x114514; // 占位地址，实际由引导程序设置

lazy_static::lazy_static! {
    /// 设备树
    pub static ref FDT: Fdt<'static> = {
        unsafe {
            let addr = Paddr::to_vaddr(&Paddr::from_usize(DTP));
            fdt::Fdt::from_ptr(addr.as_usize() as *mut u8).expect("Failed to parse device tree")
        }
    };
}

/// 初始化设备树
pub fn init() {
    println!(
        "[Device] devicetree of {} is initialized",
        FDT.root().model()
    );
    println!("[Device] now has {} CPU(s)", FDT.cpus().count());

    FDT.memory().regions().for_each(|region| {
        println!(
            "[Device] Memory Region: Start = {:#X}, Size = {:#X}",
            region.starting_address as usize,
            region.size.unwrap() as usize
        );
    });

    walk_dt(*FDT);
}

/// 返回 DRAM 的起始物理地址与总大小（合并所有 memory.regions）
/// # 返回值
/// * `Option<(usize, usize)>` - 返回起始地址和大小的元组，如果没有有效的内存区域则返回 None
pub fn dram_info() -> Option<(usize, usize)> {
    let mut start = usize::MAX;
    let mut end = 0usize;

    for region in FDT.memory().regions() {
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

/// 遍历设备树，查找并初始化 virtio 设备
/// # 参数
/// * `fdt` - 设备树对象
fn walk_dt(fdt: Fdt) {
    for node in fdt.all_nodes() {
        if let Some(compatible) = node.compatible() {
            if compatible.all().any(|s| s == "virtio,mmio") {
                virtio_probe(node);
            }
        }
    }
}

/// 探测并初始化 virtio 设备
/// 分析设备树节点，创建对应的 virtio 传输对象，并调用设备初始化函数
/// # 参数
/// * `node` - 设备树节点
fn virtio_probe(node: FdtNode) {
    // 分 析 reg 信 息
    if let Some(reg) = node.reg().and_then(|mut reg| reg.next()) {
        let paddr = reg.starting_address as usize;
        let size = reg.size.unwrap();
        pr_info!(
            "Device tree node {}: {:?}",
            node.name,
            node.compatible().map(Compatible::first),
        );
        //判 断 virtio 设 备 类 型
        let vaddr = current_memory_space()
            .lock()
            .map_mmio(Paddr::from_usize(paddr), size)
            .ok()
            .expect("Failed to map MMIO region");
        let header = NonNull::new(vaddr.as_usize() as *mut VirtIOHeader).unwrap();
        match unsafe { MmioTransport::new(header, size) } {
            Err(e) => pr_warn!("Error creating VirtIO MMIO transport: {}", e),
            Ok(transport) => {
                println!(
                    "[Device]Detected virtio MMIO device with vendor id {:#X}, device type {:?}, version {:?}",
                    transport.vendor_id(),
                    transport.device_type(),
                    transport.version(),
                );
                virtio_device(transport);
            }
        }
    }
}

/// 对不同的virtio设备进行进一步的初始化工作
/// # 参数
/// * `transport` - virtio 传输对象
fn virtio_device(transport: impl Transport) {
    match transport.device_type() {
        // DeviceType::Block => virtio_blk(transport),
        // DeviceType::GPU => virtio_gpu(transport),
        // DeviceType::Input => virtio_input(transport),
        // DeviceType::Network => virtio_net(transport),
        t => pr_warn!("Unrecognized virtio device: {:?}", t),
    }
}
