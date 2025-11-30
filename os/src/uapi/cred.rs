//! 用户凭证相关的常量和类型定义
//!
//! 对应 Linux 的 <sys/types.h> 和相关头文件

/// 根用户的 UID
pub const ROOT_UID: u32 = 0;

/// 根用户组的 GID
pub const ROOT_GID: u32 = 0;

/// 表示"不改变"的特殊 UID 值
///
/// 在 setresuid/setresgid 系统调用中，如果参数为此值，表示不改变对应的 ID
/// 对应 C 中的 (uid_t)-1
pub const UID_UNCHANGED: u32 = u32::MAX;

/// 表示"不改变"的特殊 GID 值
///
/// 在 setresuid/setresgid 系统调用中，如果参数为此值，表示不改变对应的 ID
/// 对应 C 中的 (gid_t)-1
pub const GID_UNCHANGED: u32 = u32::MAX;
