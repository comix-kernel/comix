//! 工具函数模块 - UTS 命名空间

use crate::arch::Arch;
use crate::arch::ArchImpl;

/// UTS 名称最大长度
pub const UTS_NAME_LEN: usize = 65;
pub const UTS_SYSNAME: &str = "Linux";
pub const UTS_RELEASE: &str = "5.10.0";
pub const UTS_VERSION: &str = "#1 SMP Mon Jan 1 00:00:00 UTC 2025";

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
    /// - sysname: "Linux"
    /// - nodename: "localhost"
    /// - release: "5.10.0"
    /// - version: "#1 SMP Mon Jan 1 00:00:00 UTC 2025"
    /// - machine: ArchImpl::name() (架构名称，如 "riscv64")
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
                let bytes = UTS_SYSNAME.as_bytes();
                buf[..bytes.len()].copy_from_slice(bytes);
                buf
            },
            release: {
                let mut buf = [0u8; 65];
                let bytes = UTS_RELEASE.as_bytes();
                buf[..bytes.len()].copy_from_slice(bytes);
                buf
            },
            version: {
                let mut buf = [0u8; 65];
                let bytes = UTS_VERSION.as_bytes();
                buf[..bytes.len()].copy_from_slice(bytes);
                buf
            },
            machine: {
                let mut buf = [0u8; 65];
                let bytes = ArchImpl::name().as_bytes();
                buf[..bytes.len()].copy_from_slice(bytes);
                buf
            },
        }
    }
}
