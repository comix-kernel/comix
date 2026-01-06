use crate::device::DeviceType;
use crate::device::net::net_device::NetDevice;
use crate::sync::SpinLock;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use smoltcp::iface::Interface;
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address};

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

/// Smoltcp 接口包装器，确保 Device 和 Interface 有相同的生命周期
pub struct SmoltcpInterface {
    device_adapter: NetDeviceAdapter,
    iface: Interface,
}

impl SmoltcpInterface {
    /// 创建新的 smoltcp 接口包装器
    fn new(device: Arc<dyn NetDevice>, mac_address: EthernetAddress) -> Self {
        let mut device_adapter = NetDeviceAdapter::new(device);

        let config =
            smoltcp::iface::Config::new(smoltcp::wire::HardwareAddress::Ethernet(mac_address));
        let current_time = crate::arch::timer::get_time_ms() as i64;
        let iface = Interface::new(
            config,
            &mut device_adapter,
            smoltcp::time::Instant::from_millis(current_time),
        );

        Self {
            device_adapter,
            iface,
        }
    }

    /// 轮询网络接口，处理接收和发送
    ///
    /// # 参数
    /// * `timestamp` - 当前时间戳
    /// * `sockets` - Socket 集合，用于处理网络协议栈的 socket 操作
    ///
    /// # 返回值
    /// 返回轮询结果，指示是否有事件被处理
    pub fn poll(
        &mut self,
        timestamp: Instant,
        sockets: &mut smoltcp::iface::SocketSet,
    ) -> smoltcp::iface::PollResult {
        self.iface
            .poll(timestamp, &mut self.device_adapter, sockets)
    }

    /// 获取可变的 smoltcp Interface 引用
    pub fn interface_mut(&mut self) -> &mut Interface {
        &mut self.iface
    }

    /// 获取不可变的 smoltcp Interface 引用
    pub fn interface(&self) -> &Interface {
        &self.iface
    }

    /// 获取可变的 device adapter 引用
    pub fn device_adapter_mut(&mut self) -> &mut NetDeviceAdapter {
        &mut self.device_adapter
    }

    /// 消费包装器，返回内部的Interface
    pub fn into_interface(self) -> Interface {
        self.iface
    }
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

/// 网络设备适配器，用于将NetDevice适配到smoltcp需要的Device trait
#[derive(Clone)]
pub struct NetDeviceAdapter {
    device: Arc<dyn NetDevice>,
    rx_buffer: [u8; 2048],
    loopback_queue: Arc<SpinLock<alloc::collections::VecDeque<alloc::vec::Vec<u8>>>>,
}

impl NetDeviceAdapter {
    /// 创建新的网络设备适配器
    pub fn new(device: Arc<dyn NetDevice>) -> Self {
        Self {
            device,
            rx_buffer: [0; 2048],
            loopback_queue: Arc::new(SpinLock::new(alloc::collections::VecDeque::new())),
        }
    }

    pub fn loopback_queue_len(&self) -> usize {
        self.loopback_queue.lock().len()
    }
}

// 实现smoltcp 0.12.0的Device trait
impl smoltcp::phy::Device for NetDeviceAdapter {
    type RxToken<'a> = NetRxToken<'a>;
    type TxToken<'a> = NetTxToken<'a>;

    fn receive(
        &mut self,
        _timestamp: smoltcp::time::Instant,
    ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        // 先检查 loopback 队列
        if let Some(packet) = self.loopback_queue.lock().pop_front() {
            if packet.len() > self.rx_buffer.len() {
                // Drop oversized loopback frames to avoid panicking on buffer copy.
                return None;
            }
            self.rx_buffer[..packet.len()].copy_from_slice(&packet);
            return Some((
                NetRxToken {
                    buffer: &self.rx_buffer[..packet.len()],
                },
                NetTxToken {
                    device: &self.device,
                    loopback_queue: self.loopback_queue.clone(),
                },
            ));
        }

        // 尝试从物理设备接收
        match self.device.receive(&mut self.rx_buffer) {
            Ok(size) if size > 0 => Some((
                NetRxToken {
                    buffer: &self.rx_buffer[..size],
                },
                NetTxToken {
                    device: &self.device,
                    loopback_queue: self.loopback_queue.clone(),
                },
            )),
            _ => None,
        }
    }

    fn transmit(&mut self, _timestamp: smoltcp::time::Instant) -> Option<Self::TxToken<'_>> {
        Some(NetTxToken {
            device: &self.device,
            loopback_queue: self.loopback_queue.clone(),
        })
    }

    fn capabilities(&self) -> smoltcp::phy::DeviceCapabilities {
        let mut caps = smoltcp::phy::DeviceCapabilities::default();
        // NOTE: smoltcp expects Ethernet MTU (incl. 14-byte Ethernet header, excl. 4-byte FCS).
        // Our NetDevice::mtu() follows the Linux convention (IP MTU), so we must add Ethernet header.
        caps.max_transmission_unit =
            self.device.mtu() as usize + smoltcp::wire::EthernetFrame::<&[u8]>::header_len();
        caps.medium = smoltcp::phy::Medium::Ethernet;
        // Allow the stack to process more loopback packets per poll.
        // This significantly reduces busy-wait time for high-rate workloads (e.g. iperf3 UDP).
        caps.max_burst_size = Some(64);
        caps
    }
}
/// 接收令牌
pub struct NetRxToken<'a> {
    buffer: &'a [u8],
}

impl smoltcp::phy::RxToken for NetRxToken<'_> {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(self.buffer)
    }

    fn meta(&self) -> smoltcp::phy::PacketMeta {
        smoltcp::phy::PacketMeta::default()
    }
}

/// 发送令牌
pub struct NetTxToken<'a> {
    device: &'a Arc<dyn NetDevice>,
    loopback_queue: Arc<SpinLock<alloc::collections::VecDeque<alloc::vec::Vec<u8>>>>,
}

impl smoltcp::phy::TxToken for NetTxToken<'_> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = alloc::vec![0; len];
        let result = f(&mut buffer);

        // 检查是否是 loopback 数据包。
        //
        // 特殊处理：在 NullNetDevice 上没有真实链路，发送只会被丢弃。
        // 因此将所有 Tx 都回环到 loopback_queue，确保本机协议栈自洽（iperf3 UDP/TCP）。
        let is_loopback = if self.device.name() == "null-net" {
            true
        } else if buffer.len() >= 14 {
            let ethertype = u16::from_be_bytes([buffer[12], buffer[13]]);
            match ethertype {
                0x0800 if buffer.len() >= 34 => {
                    // IP: check both source and destination IP (offset 26 and 30)
                    buffer[26] == 127 || buffer[30] == 127
                },
                0x0806 if buffer.len() >= 42 => {
                    // ARP: check both sender and target IP (offset 28 and 38)
                    buffer[28] == 127 || buffer[38] == 127
                },
                _ => false,
            }
        } else {
            false
        };

        if is_loopback {
            self.loopback_queue.lock().push_back(buffer);
        } else {
            let _ = self.device.send(&buffer);
        }

        result
    }

    fn set_meta(&mut self, _meta: smoltcp::phy::PacketMeta) {}
}

/// 实现Driver trait
impl crate::device::Driver for NetworkInterface {
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
