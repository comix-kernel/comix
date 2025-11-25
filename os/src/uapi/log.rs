//! syslog 所需的 Commands

use crate::uapi::errno::EINVAL;

/// syslog 系统调用操作类型
///
/// 与 Linux 内核 `SYSLOG_ACTION_*` 常量完全兼容。
/// 使用 enum 而非原始常量以提供类型安全和编译器优化。
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyslogAction {
    /// 关闭日志 (NOP)
    ///
    /// Linux 中此操作为空操作，保留用于历史兼容性。
    Close = 0,

    /// 打开日志 (NOP)
    ///
    /// Linux 中此操作为空操作，保留用于历史兼容性。
    Open = 1,

    /// 从日志读取（破坏性）
    ///
    /// 读取内核日志并从缓冲区移除已读条目。
    /// 需要：bufp != NULL, len > 0
    /// 权限：CAP_SYSLOG 或 CAP_SYS_ADMIN
    Read = 2,

    /// 读取所有日志（非破坏性）
    ///
    /// 读取内核日志但不从缓冲区移除。
    /// 需要：bufp != NULL, len > 0
    /// 权限：如果 dmesg_restrict=0，允许非特权访问
    ReadAll = 3,

    /// 读取并清空
    ///
    /// 先读取日志，然后清空缓冲区。
    /// 需要：bufp != NULL, len > 0
    /// 权限：CAP_SYSLOG 或 CAP_SYS_ADMIN
    ReadClear = 4,

    /// 清空缓冲区
    ///
    /// 清除所有已缓冲的日志条目。
    /// 权限：CAP_SYSLOG 或 CAP_SYS_ADMIN
    Clear = 5,

    /// 禁用控制台输出
    ///
    /// 将 console_loglevel 设置为最小值（只显示 EMERG）。
    /// 权限：CAP_SYSLOG 或 CAP_SYS_ADMIN
    ConsoleOff = 6,

    /// 启用控制台输出
    ///
    /// 恢复控制台输出到默认级别（通常为 WARNING）。
    /// 权限：CAP_SYSLOG 或 CAP_SYS_ADMIN
    ConsoleOn = 7,

    /// 设置控制台日志级别
    ///
    /// 设置 console_loglevel，控制哪些日志显示在控制台。
    /// len 参数范围：1-8
    /// 权限：CAP_SYSLOG 或 CAP_SYS_ADMIN
    ConsoleLevel = 8,

    /// 获取未读字节数
    ///
    /// 返回当前缓冲区中未读日志的估计字节数。
    /// 权限：如果 dmesg_restrict=0，允许非特权访问
    SizeUnread = 9,

    /// 获取缓冲区总大小
    ///
    /// 返回内核日志缓冲区的总容量（字节）。
    /// 权限：如果 dmesg_restrict=0，允许非特权访问
    SizeBuffer = 10,
}

impl SyslogAction {
    /// 从原始 i32 值转换为 SyslogAction
    #[inline]
    pub const fn from_i32(value: i32) -> Result<Self, i32> {
        match value {
            0 => Ok(Self::Close),
            1 => Ok(Self::Open),
            2 => Ok(Self::Read),
            3 => Ok(Self::ReadAll),
            4 => Ok(Self::ReadClear),
            5 => Ok(Self::Clear),
            6 => Ok(Self::ConsoleOff),
            7 => Ok(Self::ConsoleOn),
            8 => Ok(Self::ConsoleLevel),
            9 => Ok(Self::SizeUnread),
            10 => Ok(Self::SizeBuffer),
            _ => Err(EINVAL),
        }
    }

    /// 检查操作是否需要有效的用户缓冲区
    ///
    /// # 返回值
    ///
    /// * `true` - 需要 bufp != NULL 且 len > 0
    /// * `false` - 忽略 bufp 和 len 参数
    #[inline]
    pub const fn requires_buffer(self) -> bool {
        matches!(self, Self::Read | Self::ReadAll | Self::ReadClear)
    }

    /// 检查操作是否需要特权
    ///
    /// 注意：ReadAll 和 SizeBuffer 在 dmesg_restrict=0 时允许非特权访问。
    ///
    /// # 返回值
    ///
    /// * `true` - 总是需要权限检查
    /// * `false` - 可能允许非特权访问（需要进一步检查 dmesg_restrict）
    #[inline]
    pub const fn requires_privilege(self) -> bool {
        !matches!(self, Self::ReadAll | Self::SizeBuffer)
    }

    /// 检查操作是否会修改日志缓冲区
    ///
    /// 用于并发控制和日志审计。
    #[inline]
    pub const fn is_destructive(self) -> bool {
        matches!(self, Self::Read | Self::ReadClear | Self::Clear)
    }

    /// 检查操作是否影响控制台输出
    #[inline]
    pub const fn affects_console(self) -> bool {
        matches!(
            self,
            Self::ConsoleOff | Self::ConsoleOn | Self::ConsoleLevel
        )
    }

    /// 获取操作的字符串描述（用于日志和调试）
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Close => "CLOSE",
            Self::Open => "OPEN",
            Self::Read => "READ",
            Self::ReadAll => "READ_ALL",
            Self::ReadClear => "READ_CLEAR",
            Self::Clear => "CLEAR",
            Self::ConsoleOff => "CONSOLE_OFF",
            Self::ConsoleOn => "CONSOLE_ON",
            Self::ConsoleLevel => "CONSOLE_LEVEL",
            Self::SizeUnread => "SIZE_UNREAD",
            Self::SizeBuffer => "SIZE_BUFFER",
        }
    }
}
