use crate::kernel::task::CapabilitySet;
use crate::uapi::cred::{ROOT_GID, ROOT_UID};

/// 进程凭证结构
#[derive(Clone, Copy, Debug)]
pub struct Credential {
    /// 真实用户 ID
    pub uid: u32,
    /// 真实组 ID
    pub gid: u32,
    /// 有效用户 ID（用于权限检查）
    pub euid: u32,
    /// 有效组 ID（用于权限检查）
    pub egid: u32,
    /// 保存的用户 ID（用于 setuid 程序）
    pub suid: u32,
    /// 保存的组 ID（用于 setgid 程序）
    pub sgid: u32,
    /// 文件系统用户 ID（Linux 扩展，用于文件系统操作）
    pub fsuid: u32,
    /// 文件系统组 ID
    pub fsgid: u32,

    /// 能力集合
    pub capabilities: CapabilitySet,
}

impl Credential {
    /// 创建 root 用户凭证
    pub const fn root() -> Self {
        Self {
            uid: ROOT_UID,
            gid: ROOT_GID,
            euid: ROOT_UID,
            egid: ROOT_GID,
            suid: ROOT_UID,
            sgid: ROOT_GID,
            fsuid: ROOT_UID,
            fsgid: ROOT_GID,
            capabilities: CapabilitySet::full(),
        }
    }

    /// 检查是否为 root 用户
    pub fn is_root(&self) -> bool {
        self.euid == ROOT_UID
    }
}
