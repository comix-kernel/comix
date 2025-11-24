//! 网络子系统模块
//!
//! 此模块实现了操作系统的网络栈，包括网络接口抽象、协议栈实现
//! 以及网络系统调用支持。

use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
pub mod interface;
pub mod protocol;
pub mod stack;

use crate::println;

use self::interface::NetworkInterface;

/// 网络接口错误类型
#[derive(Debug, Clone, PartialEq)]
pub enum NetworkError {
    /// 接口未启用
    InterfaceDisabled,
    /// 接口不存在
    InterfaceNotFound,
    /// 发送缓冲区已满
    SendBufferFull,
    /// 接收缓冲区为空
    ReceiveBufferEmpty,
    /// 无效的配置参数
    InvalidConfig,
    /// 设备错误
    DeviceError(String),
    /// 协议错误
    ProtocolError(String),
    /// 其他错误
    Other(String),
}

impl From<crate::device::net::net_device::NetDeviceError> for NetworkError {
    fn from(err: crate::device::net::net_device::NetDeviceError) -> Self {
        match err {
            crate::device::net::net_device::NetDeviceError::IoError => {
                NetworkError::DeviceError("I/O error".to_string())
            }
            crate::device::net::net_device::NetDeviceError::DeviceNotReady => {
                NetworkError::DeviceError("Device not ready".to_string())
            }
            crate::device::net::net_device::NetDeviceError::NotSupported => {
                NetworkError::DeviceError("Operation not supported".to_string())
            }
            crate::device::net::net_device::NetDeviceError::QueueFull => {
                NetworkError::SendBufferFull
            }
            crate::device::net::net_device::NetDeviceError::QueueEmpty => {
                NetworkError::ReceiveBufferEmpty
            }
            crate::device::net::net_device::NetDeviceError::AllocationFailed => {
                NetworkError::DeviceError("Memory allocation failed".to_string())
            }
        }
    }
}

/// 运行网络测试
pub fn run_tests() {
    println!("[Network] Running network tests...");

    // 这里可以添加网络相关的测试代码
    println!("[Network] Network tests completed");
}
