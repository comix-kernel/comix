use crate::{
    device::{NETWORK_DEVICES, NetDevice},
    println,
};
use alloc::vec::Vec;
// use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address};

pub mod net_device;
pub mod network_config;
pub mod network_interface;
pub mod virtio_net;

/// 注册网络设备
pub fn register_net_device(device: alloc::sync::Arc<dyn NetDevice>) {
    // 将克隆的设备添加到列表中
    NETWORK_DEVICES.lock().push(device.clone());

    // 最后输出注册信息（使用克隆的引用）
    println!("Network device registered: {}", device.name());
}

/// 获取所有网络设备
pub fn get_net_devices() -> Vec<alloc::sync::Arc<dyn NetDevice>> {
    NETWORK_DEVICES.lock().clone()
}

/// 初始化网络设备
/// 在系统启动时调用
pub fn init_net_devices() {
    println!("Initializing network devices...");

    // 尝试探测和初始化 VirtIO 网络设备
    // 暂时注释掉直接访问设备的代码，避免内核崩溃
    const VIRTIO_NET_MMIO_ADDR: usize = 0x1000_0000;

    println!("Skipping VirtIO network device initialization for now");
    println!(
        "Device address: {:#x} needs proper memory mapping first",
        VIRTIO_NET_MMIO_ADDR
    );

    /*
    // 未来实现：在正确映射设备地址空间后启用以下代码
    unsafe {
        // 1. 首先需要将设备物理地址映射到虚拟地址空间
        // 2. 然后使用映射后的虚拟地址访问设备
        // let mapped_virt_addr = map_device_to_virt(VIRTIO_NET_MMIO_ADDR, 0x1000);
        // let header_ptr = NonNull::new(mapped_virt_addr as *mut VirtIOHeader)
        //     .ok_or("Failed to create valid device pointer")?;

        // 尝试初始化MmioTransport，第二个参数是设备地址的大小（假设为4KB）
        // match MmioTransport::new(header_ptr, 0x1000) {
        //     Ok(transport) => {
        //         // 尝试创建 VirtioNetDevice
        //         match crate::devices::net_device::VirtioNetDevice::new(transport, 0) {
        //             Ok(device) => {
        //                 println!("Found VirtIO network device at {:#x}", VIRTIO_NET_MMIO_ADDR);
        //                 println!("MAC Address: {}", format_mac_address(device.mac_address()));
        //                 println!("MTU: {}", device.mtu());
        //
        //                 // 注册网络设备
        //                 register_net_device(device);
        //             },
        //             Err(_) => println!("Failed to initialize VirtIO network device"),
        //         }
        //     },
        //     Err(_) => {
        //         println!("No VirtIO network device detected");
        //     }
        // }
    }
    */
}

/// 格式化MAC地址为可读字符串
fn format_mac_address(mac: [u8; 6]) -> alloc::string::String {
    use alloc::format;
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}
