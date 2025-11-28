//! 工具函数模块 - UTS 命名空间

use crate::arch::constant::ARCH;

/// UTS 名称最大长度
pub const UTS_NAME_LEN: usize = 65;

/// UTS 命名空间结构体
/// 用于隔离不同任务的主机名和域名设置
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UtsNamespace {
    /// 系统名称
    pub sysname: [u8; UTS_NAME_LEN],
    /// 主机名
    pub nodename: [u8; UTS_NAME_LEN],
    /// 发行版版本
    pub release: [u8; UTS_NAME_LEN],
    /// 版本信息
    pub version: [u8; UTS_NAME_LEN],
    /// 机器类型
    pub machine: [u8; UTS_NAME_LEN],
    /// 域名
    pub domainname: [u8; UTS_NAME_LEN],
}

impl Default for UtsNamespace {
    /// 创建一个默认的 UTS 命名空间实例
    ///
    /// 默认值为：
    /// - sysname: "ComixOS"
    /// - nodename: "localhost"
    /// - release: "0.1.0"
    /// - version: "Version 0.1.0"
    /// - machine: ARCH (架构名称)
    /// - domainname: "localdomain"
    fn default() -> Self {
        Self {
            nodename: {
                let mut buf = [0u8; 65];
                let bytes = "localhost".as_bytes();
                buf[..bytes.len()].copy_from_slice(bytes);
                buf
            },
            domainname: {
                let mut buf = [0u8; 65];
                let bytes = "localdomain".as_bytes();
                buf[..bytes.len()].copy_from_slice(bytes);
                buf
            },
            sysname: {
                let mut buf = [0u8; 65];
                let bytes = "ComixOS".as_bytes();
                buf[..bytes.len()].copy_from_slice(bytes);
                buf
            },
            release: {
                let mut buf = [0u8; 65];
                let bytes = "0.1.0".as_bytes();
                buf[..bytes.len()].copy_from_slice(bytes);
                buf
            },
            version: {
                let mut buf = [0u8; 65];
                let bytes = "Version 0.1.0".as_bytes();
                buf[..bytes.len()].copy_from_slice(bytes);
                buf
            },
            machine: {
                let mut buf = [0u8; 65];
                let bytes = ARCH.as_bytes();
                buf[..bytes.len()].copy_from_slice(bytes);
                buf
            },
        }
    }
}
