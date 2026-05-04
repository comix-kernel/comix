use crate::device::DeviceType;
use crate::device::net::net_device::NetDevice;
use crate::sync::SpinLock;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, IpCidr, Ipv4Address};

pub use crate::net::stack::{NetDeviceAdapter, SmoltcpInterface};

/// 网络接口管理器
pub struct NetworkInterfaceManager {
    interfaces: Vec<Arc<NetworkInterface>>,
}

impl NetworkInterfaceManager {
    /// 创建新的网络接口管理器
    pub fn new() -> Self {
        Self {
            interfaces: Vec::new(),
        }
    }

    /// 添加网络接口
    pub fn add_interface(&mut self, interface: Arc<NetworkInterface>) {
        self.interfaces.push(interface);
    }

    /// 获取所有网络接口
    pub fn get_interfaces(&self) -> &[Arc<NetworkInterface>] {
        &self.interfaces
    }

    /// 通过名称查找网络接口
    pub fn find_interface_by_name(&self, name: &str) -> Option<&Arc<NetworkInterface>> {
        self.interfaces.iter().find(|iface| iface.name() == name)
    }
}

lazy_static! {
    /// 全局网络接口管理器
    pub static ref NETWORK_INTERFACE_MANAGER: SpinLock<NetworkInterfaceManager> =
        SpinLock::new(NetworkInterfaceManager::new());
}

/// 网络接口
pub struct NetworkInterface {
    name: String,
    mac_address: EthernetAddress,
    device: Arc<dyn NetDevice>,
    ip_addresses: SpinLock<Vec<IpCidr>>,
    ipv4_gateway: SpinLock<Option<Ipv4Address>>,
    interrupt_enabled: SpinLock<bool>,
    last_interrupt_time: SpinLock<Instant>,
}

impl NetworkInterface {
    /// 创建新的网络接口
    pub fn new(name: String, device: Arc<dyn NetDevice>) -> Self {
        let mac_address = EthernetAddress(device.mac_address());
        Self {
            name,
            mac_address,
            device,
            ip_addresses: SpinLock::new(Vec::new()),
            ipv4_gateway: SpinLock::new(None),
            interrupt_enabled: SpinLock::new(true),
            last_interrupt_time: SpinLock::new(Instant::from_millis(0)),
        }
    }

    /// 获取接口名称
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 获取MAC地址
    pub fn mac_address(&self) -> EthernetAddress {
        self.mac_address
    }

    /// 获取底层网络设备
    pub fn device(&self) -> &Arc<dyn NetDevice> {
        &self.device
    }

    /// 设置IP地址
    pub fn add_ip_address(&self, ip_cidr: IpCidr) {
        let mut ip_addresses = self.ip_addresses.lock();
        if !ip_addresses.contains(&ip_cidr) {
            ip_addresses.push(ip_cidr);
        }
    }

    /// 获取所有IP地址
    pub fn ip_addresses(&self) -> Vec<IpCidr> {
        self.ip_addresses.lock().clone()
    }

    /// 设置IPv4网关
    pub fn set_ipv4_gateway(&self, gateway: Option<Ipv4Address>) {
        *self.ipv4_gateway.lock() = gateway;
    }

    /// 获取IPv4网关
    pub fn ipv4_gateway(&self) -> Option<Ipv4Address> {
        *self.ipv4_gateway.lock()
    }

    /// 启用中断
    pub fn enable_interrupt(&self) {
        *self.interrupt_enabled.lock() = true;
        crate::pr_debug!("Interrupt enabled for interface {}", self.name());
    }

    /// 禁用中断
    pub fn disable_interrupt(&self) {
        *self.interrupt_enabled.lock() = false;
        crate::pr_debug!("Interrupt disabled for interface {}", self.name());
    }

    /// 检查中断是否启用
    pub fn is_interrupt_enabled(&self) -> bool {
        *self.interrupt_enabled.lock()
    }

    /// 更新最后中断时间
    pub fn update_interrupt_time(&self) {
        // 在实际系统中，这里应该使用真实的时间源
        // 这里使用一个简单的计数器模拟
        *self.last_interrupt_time.lock() =
            Instant::from_millis(self.last_interrupt_time.lock().total_millis() + 1);
    }

    /// 创建smoltcp以太网接口
    ///
    /// 返回一个 SmoltcpInterface 包装器，它拥有 NetDeviceAdapter 和 Interface，
    /// 确保两者有相同的生命周期，避免悬垂指针问题。
    pub fn create_smoltcp_interface(&self) -> SmoltcpInterface {
        // 创建包装器（内部会创建 device_adapter 和 interface）
        let mut smoltcp_iface = SmoltcpInterface::new(self.device.clone(), self.mac_address());

        // 设置IP地址
        for ip_cidr in self.ip_addresses.lock().iter() {
            smoltcp_iface.interface_mut().update_ip_addrs(|addrs| {
                let _ = addrs.push(*ip_cidr);
            });
        }

        // 设置路由
        if let Some(gateway) = self.ipv4_gateway() {
            smoltcp_iface
                .interface_mut()
                .routes_mut()
                .add_default_ipv4_route(gateway)
                .ok();
        }

        smoltcp_iface
    }
}

/// 实现Driver trait
/// Compatibility driver handle for net-device interrupt registration.
///
/// Interface configuration stays in `NetworkInterface`; this shim keeps the
/// legacy device registry working while the network subsystem owns interfaces.
pub struct NetDriverHandle {
    interface: Arc<NetworkInterface>,
}

impl NetDriverHandle {
    pub fn new(interface: Arc<NetworkInterface>) -> Self {
        Self { interface }
    }
}

impl core::ops::Deref for NetDriverHandle {
    type Target = NetworkInterface;

    fn deref(&self) -> &Self::Target {
        &self.interface
    }
}

impl crate::device::Driver for NetDriverHandle {
    fn try_handle_interrupt(&self, irq: Option<usize>) -> bool {
        // 检查中断是否启用
        if !self.is_interrupt_enabled() {
            return false;
        }

        // 记录中断信息
        if let Some(irq_num) = irq {
            crate::pr_debug!(
                "Network interface {} received interrupt on IRQ {}",
                self.name(),
                irq_num
            );
        } else {
            crate::pr_debug!(
                "Network interface {} received interrupt without IRQ number",
                self.name()
            );
        }

        // 更新最后中断时间
        self.update_interrupt_time();

        // 处理接收中断：尝试接收数据包
        let mut buffer = [0u8; 2048];
        match self.device.receive(&mut buffer) {
            Ok(size) if size > 0 => {
                crate::pr_debug!(
                    "Received {} bytes of data on interface {}",
                    size,
                    self.name()
                );
                // Wake up tasks waiting in ppoll
                crate::kernel::syscall::io::wake_poll_waiters();
                true
            }
            Err(e) => {
                crate::pr_debug!("Error receiving data: {:?}", e);
                true // 仍然返回true表示我们处理了这个中断
            }
            _ => {
                // 没有接收到数据，但可能是发送完成中断等
                true
            }
        }
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Net
    }

    fn get_id(&self) -> alloc::string::String {
        self.name.clone()
    }

    fn as_net(&self) -> Option<&dyn crate::device::net::net_device::NetDevice> {
        Some(self.device.as_ref())
    }

    fn as_net_arc(self: Arc<Self>) -> Option<Arc<dyn crate::device::net::net_device::NetDevice>> {
        Some(self.device.clone())
    }

    fn as_block(&self) -> Option<&dyn crate::device::block::BlockDriver> {
        None
    }

    fn as_rtc(&self) -> Option<&dyn crate::device::rtc::RtcDriver> {
        None
    }
}
