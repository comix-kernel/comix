//! 工具函数模块 - UTS 命名空间

use alloc::vec::Vec;

pub const HOST_NAME_MAX: usize = 64;
pub const UTS_DOMAINNAME_MAX_LEN: usize = 64;

/// UTS 命名空间结构体
/// 用于隔离不同任务的主机名和域名设置
#[derive(Debug)]
pub struct UtsNamespace {
    /// 主机名
    pub nodename: Vec<u8>,
    /// 域名
    pub domainname: Vec<u8>,
}

impl Default for UtsNamespace {
    fn default() -> Self {
        Self {
            nodename: "localhost".as_bytes().to_vec(),
            domainname: "localdomain".as_bytes().to_vec(),
        }
    }
}
