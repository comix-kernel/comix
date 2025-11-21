use crate::device::net::network_interface::NETWORK_INTERFACE_MANAGER;
use crate::println;
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
    /// 初始化默认网络接口配置
    pub fn init_default_interface() -> Result<(), NetworkConfigError> {
        println!("Initializing default network configuration...");

        // 查找名为"eth0"的网络接口
        let binding = NETWORK_INTERFACE_MANAGER.lock();
        let interfaces = binding.get_interfaces();

        if let Some(interface) = interfaces.first() {
            println!("Configuring interface: {}", interface.name());

            // 设置默认IP地址
            let ip_cidr = IpCidr::new(IpAddress::v4(192, 168, 1, 100), 24);
            interface.add_ip_address(ip_cidr);
            println!("Set IP address: 192.168.1.100/24");

            // 设置默认网关
            let gateway = Ipv4Address::new(192, 168, 1, 1);
            interface.set_ipv4_gateway(Some(gateway));
            println!("Set default gateway: 192.168.1.1");

            Ok(())
        } else {
            println!("No network interfaces found to configure");
            Err(NetworkConfigError::InterfaceNotFound)
        }
    }

    /// 设置网络接口IP地址
    pub fn set_ip_address(
        interface_name: &str,
        ip: &str,
        prefix: u8,
    ) -> Result<(), NetworkConfigError> {
        // 查找指定接口
        if let Some(interface) = NETWORK_INTERFACE_MANAGER
            .lock()
            .find_interface_by_name(interface_name)
        {
            // 解析IP地址
            match ip.parse::<Ipv4Address>() {
                Ok(ipv4) => {
                    let ip_cidr = IpCidr::new(IpAddress::Ipv4(ipv4), prefix);
                    interface.add_ip_address(ip_cidr);
                    println!("Set IP address for {}: {}/{}", interface_name, ip, prefix);
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
        // 查找指定接口
        if let Some(interface) = NETWORK_INTERFACE_MANAGER
            .lock()
            .find_interface_by_name(interface_name)
        {
            // 解析网关地址
            match gateway.parse::<Ipv4Address>() {
                Ok(gateway_ipv4) => {
                    interface.set_ipv4_gateway(Some(gateway_ipv4));
                    println!("Set default gateway for {}: {}", interface_name, gateway);
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
        if let Some(interface) = NETWORK_INTERFACE_MANAGER
            .lock()
            .find_interface_by_name(interface_name)
        {
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
        // 查找指定接口
        if let Some(interface) = NETWORK_INTERFACE_MANAGER
            .lock()
            .find_interface_by_name(interface_name)
        {
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
            let prefix_length = match mask {
                "255.255.255.0" => 24,
                "255.255.0.0" => 16,
                "255.0.0.0" => 8,
                "255.255.255.128" => 25,
                "255.255.255.192" => 26,
                "255.255.255.224" => 27,
                "255.255.255.240" => 28,
                "255.255.255.248" => 29,
                "255.255.255.252" => 30,
                _ => return Err(NetworkConfigError::InvalidSubnet),
            };

            // 设置IP地址
            let ip_cidr = IpCidr::new(IpAddress::Ipv4(ip_address), prefix_length);
            interface.add_ip_address(ip_cidr);

            // 设置网关
            interface.set_ipv4_gateway(Some(gateway_address));

            println!(
                "Set interface config for {}: IP={}/{}, Gateway={}",
                interface_name, ip, prefix_length, gateway
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
