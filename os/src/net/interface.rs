//! 网络接口抽象模块
//!
//! 此模块定义了网络接口的抽象接口，用于管理不同类型的网络接口。

use alloc::{
    string::{String, ToString},
    sync::Arc,
};
use spin::Mutex;

use crate::net::device::NetDevice;

/// 网络接口配置
pub struct InterfaceConfig {
    /// 接口名称
    pub name: String,
    /// MAC 地址
    pub mac_address: [u8; 6],
    /// IP 地址 (IPv4)
    pub ip_address: [u8; 4],
    /// 子网掩码
    pub netmask: [u8; 4],
    /// 默认网关
    pub gateway: [u8; 4],
    /// 接口是否启用
    pub enabled: bool,
}

impl InterfaceConfig {
    /// 创建一个新的接口配置
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            mac_address: [0; 6],
            ip_address: [0; 4],
            netmask: [0; 4],
            gateway: [0; 4],
            enabled: false,
        }
    }
}

/// 网络接口统计信息
pub struct InterfaceStats {
    /// 发送的数据包数量
    pub tx_packets: usize,
    /// 接收的数据包数量
    pub rx_packets: usize,
    /// 发送的字节数
    pub tx_bytes: usize,
    /// 接收的字节数
    pub rx_bytes: usize,
    /// 发送错误数
    pub tx_errors: usize,
    /// 接收错误数
    pub rx_errors: usize,
}

impl InterfaceStats {
    /// 创建一个新的统计信息实例
    pub fn new() -> Self {
        Self {
            tx_packets: 0,
            rx_packets: 0,
            tx_bytes: 0,
            rx_bytes: 0,
            tx_errors: 0,
            rx_errors: 0,
        }
    }
}

/// 网络接口抽象
pub struct NetworkInterface {
    /// 接口配置
    pub config: InterfaceConfig,
    /// 接口统计信息
    pub stats: InterfaceStats,
    /// 底层网络设备
    pub device: Arc<Mutex<dyn NetDevice>>,
}

impl NetworkInterface {
    /// 创建一个新的网络接口
    pub fn new(name: &str, device: Arc<Mutex<dyn NetDevice>>) -> Self {
        Self {
            config: InterfaceConfig::new(name),
            stats: InterfaceStats::new(),
            device,
        }
    }

    /// 发送数据包
    pub fn send_packet(&mut self, data: &[u8]) -> Result<(), ()> {
        if !self.config.enabled {
            return Err(());
        }

        match self.device.lock().send(data) {
            Ok(_) => {
                self.stats.tx_packets += 1;
                self.stats.tx_bytes += data.len();
                Ok(())
            }
            Err(_) => {
                self.stats.tx_errors += 1;
                Err(())
            }
        }
    }

    /// 接收数据包
    pub fn receive_packet(&mut self, buffer: &mut [u8]) -> Result<usize, ()> {
        if !self.config.enabled {
            return Err(());
        }

        match self.device.lock().receive(buffer) {
            Ok(size) => {
                self.stats.rx_packets += 1;
                self.stats.rx_bytes += size;
                Ok(size)
            }
            Err(_) => {
                self.stats.rx_errors += 1;
                Err(())
            }
        }
    }

    /// 设置接口配置
    pub fn set_config(&mut self, new_config: InterfaceConfig) {
        self.config = new_config;
    }

    /// 获取接口统计信息
    pub fn get_stats(&self) -> &InterfaceStats {
        &self.stats
    }

    /// 启用接口
    pub fn enable(&mut self) {
        self.config.enabled = true;
    }

    /// 禁用接口
    pub fn disable(&mut self) {
        self.config.enabled = false;
    }
}
