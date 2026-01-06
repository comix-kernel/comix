use crate::{
    earlyprintln,
    net::interface::{NETWORK_INTERFACE_MANAGER, NetworkInterface},
};
use alloc::string::String;
use smoltcp::wire::{IpAddress, IpCidr, Ipv4Address};

/// 网络配置错误
#[derive(Debug)]
pub enum NetworkConfigError {
    InterfaceNotFound,
    InvalidAddress,
    InvalidSubnet,
    InvalidGateway,
    ConfigFailed,
}

/// 网络配置管理器
pub struct NetworkConfigManager;

impl NetworkConfigManager {
    /// 解析点分十进制子网掩码并计算前缀长度
    ///
    /// # 参数
    /// * `mask` - 点分十进制格式的子网掩码字符串 (如 "255.255.255.0")
    ///
    /// # 返回值
    /// * `Ok(u8)` - 成功时返回前缀长度 (0-32)
    /// * `Err(NetworkConfigError)` - 失败时返回错误
    ///
    /// # 示例
    /// ```
    /// parse_subnet_mask("255.255.255.0") // 返回 Ok(24)
    /// parse_subnet_mask("255.255.0.0")   // 返回 Ok(16)
    /// parse_subnet_mask("255.255.255.128") // 返回 Ok(25)
    /// parse_subnet_mask("255.255.255.3") // 返回 Err (无效掩码)
    /// ```
    fn parse_subnet_mask(mask: &str) -> Result<u8, NetworkConfigError> {
        // 解析点分十进制字符串为4个字节
        let octets: Result<alloc::vec::Vec<u8>, _> =
            mask.split('.').map(|s| s.parse::<u8>()).collect();

        let octets = octets.map_err(|_| NetworkConfigError::InvalidSubnet)?;

        // 必须是4个字节
        if octets.len() != 4 {
            return Err(NetworkConfigError::InvalidSubnet);
        }

        // 将4个字节转换为32位整数
        let mask_u32 = ((octets[0] as u32) << 24)
            | ((octets[1] as u32) << 16)
            | ((octets[2] as u32) << 8)
            | (octets[3] as u32);

        // 验证掩码的有效性：必须是连续的1后跟连续的0
        // 例如: 11111111111111111111111100000000 (0xFFFFFF00) 是有效的
        //       11111111111111110000000011111111 (0xFFFF00FF) 是无效的

        // 计算前缀长度（前导1的个数）
        let prefix_length = mask_u32.leading_ones() as u8;

        // 验证：如果有 n 个前导1，那么掩码应该等于 (0xFFFFFFFF << (32 - n))
        // 这确保了所有的1都是连续的
        if prefix_length == 0 {
            // 特殊情况：掩码为 0.0.0.0
            if mask_u32 == 0 {
                return Ok(0);
            } else {
                return Err(NetworkConfigError::InvalidSubnet);
            }
        } else if prefix_length == 32 {
            // 特殊情况：掩码为 255.255.255.255
            if mask_u32 == 0xFFFFFFFF {
                return Ok(32);
            } else {
                return Err(NetworkConfigError::InvalidSubnet);
            }
        } else {
            // 一般情况：验证掩码格式
            let expected_mask = 0xFFFFFFFFu32 << (32 - prefix_length);
            if mask_u32 != expected_mask {
                return Err(NetworkConfigError::InvalidSubnet);
            }
            return Ok(prefix_length);
        }
    }

    /// 初始化默认网络接口配置
    pub fn init_default_interface() -> Result<(), NetworkConfigError> {
        earlyprintln!("Initializing default network configuration...");

        // 先获取接口的Arc，然后释放全局锁
        // 避免在持有 NETWORK_INTERFACE_MANAGER 锁时操作接口字段锁
        let interface = {
            let binding = NETWORK_INTERFACE_MANAGER.lock();
            binding.get_interfaces().first().cloned()
        }; // NETWORK_INTERFACE_MANAGER锁已释放

        let (interface, is_null_loopback_only) = if let Some(interface) = interface {
            (interface, false)
        } else {
            // 没有任何真实网卡（例如 QEMU 没挂 virtio-net）时，创建一个“空设备接口”，
            // 让 smoltcp/套接字栈至少能在 loopback(127.0.0.1) 场景工作。
            use crate::device::net::null_net::NullNetDevice;
            use alloc::sync::Arc;

            earlyprintln!("No net interfaces found; creating null loopback-only interface");

            let dev = NullNetDevice::new(0);
            let iface = Arc::new(NetworkInterface::new(String::from("lo0"), dev));
            NETWORK_INTERFACE_MANAGER.lock().add_interface(iface.clone());
            (iface, true)
        };

        {
            earlyprintln!("Configuring interface: {}", interface.name());

            if is_null_loopback_only {
                // NullNetDevice 只有 loopback 场景可用：不要配置非 127/8 的地址和网关，
                // 否则 smoltcp 可能会为 127.0.0.1 选择错误的源地址/路由，导致 UDP/TCP 行为异常。
                let loopback_cidr = IpCidr::new(IpAddress::v4(127, 0, 0, 1), 8);
                interface.add_ip_address(loopback_cidr);
                earlyprintln!("Set loopback address: 127.0.0.1/8");
                interface.set_ipv4_gateway(None);
                earlyprintln!("No default gateway (loopback-only)");
            } else {
                // 设置默认IP地址
                let ip_cidr = IpCidr::new(IpAddress::v4(192, 168, 1, 100), 24);
                interface.add_ip_address(ip_cidr);
                earlyprintln!("Set IP address: 192.168.1.100/24");

                // 添加loopback地址到同一接口
                let loopback_cidr = IpCidr::new(IpAddress::v4(127, 0, 0, 1), 8);
                interface.add_ip_address(loopback_cidr);
                earlyprintln!("Set loopback address: 127.0.0.1/8");

                // 设置默认网关
                let gateway = Ipv4Address::new(192, 168, 1, 1);
                interface.set_ipv4_gateway(Some(gateway));
                earlyprintln!("Set default gateway: 192.168.1.1");
            }

            // Initialize global interface for socket operations
            let mut smoltcp_iface = interface.create_smoltcp_interface();
            {
                use smoltcp::phy::Device as _;
                let caps = smoltcp_iface.device_adapter_mut().capabilities();
                earlyprintln!(
                    "smoltcp caps: medium={:?}, max_transmission_unit={}, ip_mtu={}",
                    caps.medium,
                    caps.max_transmission_unit,
                    caps.ip_mtu()
                );
            }
            use crate::net::socket::init_network;
            init_network(smoltcp_iface);
            earlyprintln!("Initialized global network interface");

            Ok(())
        }
    }

    /// 设置网络接口IP地址
    pub fn set_ip_address(
        interface_name: &str,
        ip: &str,
        prefix: u8,
    ) -> Result<(), NetworkConfigError> {
        // 先获取接口的Arc，然后释放全局锁
        let interface = NETWORK_INTERFACE_MANAGER
            .lock()
            .find_interface_by_name(interface_name)
            .cloned(); // clone Arc，然后锁自动释放

        if let Some(interface) = interface {
            // 解析IP地址
            match ip.parse::<Ipv4Address>() {
                Ok(ipv4) => {
                    let ip_cidr = IpCidr::new(IpAddress::Ipv4(ipv4), prefix);
                    interface.add_ip_address(ip_cidr);
                    earlyprintln!("Set IP address for {}: {}/{}", interface_name, ip, prefix);
                    Ok(())
                }
                Err(_) => Err(NetworkConfigError::InvalidAddress),
            }
        } else {
            Err(NetworkConfigError::InterfaceNotFound)
        }
    }

    /// 设置默认网关
    pub fn set_default_gateway(
        interface_name: &str,
        gateway: &str,
    ) -> Result<(), NetworkConfigError> {
        // 先获取接口的Arc，然后释放全局锁
        let interface = NETWORK_INTERFACE_MANAGER
            .lock()
            .find_interface_by_name(interface_name)
            .cloned(); // clone Arc，然后锁自动释放

        if let Some(interface) = interface {
            // 解析网关地址
            match gateway.parse::<Ipv4Address>() {
                Ok(gateway_ipv4) => {
                    interface.set_ipv4_gateway(Some(gateway_ipv4));
                    earlyprintln!("Set default gateway for {}: {}", interface_name, gateway);
                    Ok(())
                }
                Err(_) => Err(NetworkConfigError::InvalidGateway),
            }
        } else {
            Err(NetworkConfigError::InterfaceNotFound)
        }
    }

    /// 获取网络接口配置信息
    pub fn get_interface_config(interface_name: &str) -> Result<String, NetworkConfigError> {
        // 先获取接口的Arc，然后释放全局锁
        let interface = NETWORK_INTERFACE_MANAGER
            .lock()
            .find_interface_by_name(interface_name)
            .cloned(); // clone Arc，然后锁自动释放

        if let Some(interface) = interface {
            let mut config = alloc::format!("Interface: {}\n", interface.name());
            config.push_str(&alloc::format!(
                "MAC Address: {}\n",
                interface.mac_address()
            ));

            // 添加IP地址信息
            let ip_addresses = interface.ip_addresses();
            if !ip_addresses.is_empty() {
                config.push_str("IP Addresses:\n");
                for ip in ip_addresses {
                    config.push_str(&alloc::format!("  {}\n", ip));
                }
            } else {
                config.push_str("No IP addresses configured\n");
            }

            // 添加网关信息
            if let Some(gateway) = interface.ipv4_gateway() {
                config.push_str(&alloc::format!("Default Gateway: {}\n", gateway));
            } else {
                config.push_str("No default gateway configured\n");
            }

            Ok(config)
        } else {
            Err(NetworkConfigError::InterfaceNotFound)
        }
    }

    /// 设置网络接口配置
    pub fn set_interface_config(
        interface_name: &str,
        ip: &str,
        gateway: &str,
        mask: &str,
    ) -> Result<(), NetworkConfigError> {
        // 先获取接口的Arc，然后释放全局锁
        let interface = NETWORK_INTERFACE_MANAGER
            .lock()
            .find_interface_by_name(interface_name)
            .cloned(); // clone Arc，然后锁自动释放

        if let Some(interface) = interface {
            // 解析IP地址
            let ip_address = match ip.parse::<Ipv4Address>() {
                Ok(ipv4) => ipv4,
                Err(_) => return Err(NetworkConfigError::InvalidAddress),
            };

            // 解析网关地址
            let gateway_address = match gateway.parse::<Ipv4Address>() {
                Ok(gw) => gw,
                Err(_) => return Err(NetworkConfigError::InvalidGateway),
            };

            // 解析子网掩码，并计算前缀长度
            let prefix_length = Self::parse_subnet_mask(mask)?;

            // 设置IP地址
            let ip_cidr = IpCidr::new(IpAddress::Ipv4(ip_address), prefix_length);
            interface.add_ip_address(ip_cidr);

            // 设置网关
            interface.set_ipv4_gateway(Some(gateway_address));

            earlyprintln!(
                "Set interface config for {}: IP={}/{}, Gateway={}",
                interface_name,
                ip,
                prefix_length,
                gateway
            );
            Ok(())
        } else {
            Err(NetworkConfigError::InterfaceNotFound)
        }
    }

    /// 列出所有网络接口
    pub fn list_interfaces() -> String {
        let manager = NETWORK_INTERFACE_MANAGER.lock();
        let interfaces = manager.get_interfaces();
        let mut result = alloc::string::String::new();

        if interfaces.is_empty() {
            result.push_str("No network interfaces available\n");
        } else {
            result.push_str("Available network interfaces:\n");
            for (index, interface) in interfaces.iter().enumerate() {
                result.push_str(&alloc::format!("{}: {}\n", index + 1, interface.name()));
                result.push_str(&alloc::format!("   MAC: {}\n", interface.mac_address()));
            }
        }

        result
    }
}
