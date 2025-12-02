//! Virtio MMIO 设备驱动模块
//!
//! 提供对 Virtio MMIO 设备的探测和初始化功能
//! 通过设备树节点信息创建 Virtio 传输对象，并根据设备类型调用相应的初始化函数
//! 支持块设备、GPU、输入设备和网络设备的初始化
use core::ptr::NonNull;

use fdt::node::FdtNode;
use virtio_drivers::transport::{
    DeviceType, Transport,
    mmio::{MmioTransport, VirtIOHeader},
};

use crate::{
    device::{
        block::virtio_blk, device_tree::DEVICE_TREE_REGISTRY, gpu::virtio_gpu, input::virtio_input,
        net::virtio_net,
    },
    kernel::current_memory_space,
    mm::address::{Paddr, UsizeConvert},
    pr_info, pr_warn,
};

pub fn driver_init() {
    DEVICE_TREE_REGISTRY
        .write()
        .insert("virtio,mmio", virtio_probe);
}

/// 探测并初始化 virtio 设备
/// 分析设备树节点，创建对应的 virtio 传输对象，并调用设备初始化函数
/// # 参数
/// * `node` - 设备树节点
fn virtio_probe(node: &FdtNode) {
    // 分 析 reg 信 息
    if let Some(reg) = node.reg().and_then(|mut reg| reg.next()) {
        let paddr = reg.starting_address as usize;
        let size = reg.size.unwrap_or(0);
        if size == 0 {
            pr_warn!(
                "[Device] Virtio MMIO device tree node {} has no size",
                node.name
            );
            return;
        }
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
                virtio_device(transport);
            }
        }
    }
}

/// 对不同的virtio设备进行进一步的初始化工作
/// # 参数
/// * `transport` - virtio 传输对象
fn virtio_device(transport: MmioTransport<'static>) {
    match transport.device_type() {
        DeviceType::Block => virtio_blk::init(transport),
        DeviceType::GPU => virtio_gpu::init(transport),
        DeviceType::Input => virtio_input::init(transport),
        DeviceType::Network => virtio_net::init(transport),
        t => pr_warn!("Unrecognized virtio device: {:?}", t),
    }
}
