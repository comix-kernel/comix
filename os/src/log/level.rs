//! 日志级别定义
//!
//! 该模块定义了内核日志系统使用的**八个日志级别**，
//! 与 Linux 内核的 `printk` 级别相匹配。

/// 日志级别枚举
///
/// 定义了从 Emergency (最高优先级) 到 Debug (最低优先级) 的八个优先级。
/// 这些级别与 Linux 内核的 `KERN_*` 常量兼容。
///
/// # 级别语义
///
/// - **Emergency (紧急)**: 系统不可用
/// - **Alert (警报)**: 必须立即采取行动
/// - **Critical (关键)**: 关键状况
/// - **Error (错误)**: 错误状况
/// - **Warning (警告)**: 警告状况
/// - **Notice (通知)**: 正常但重要的状况
/// - **Info (信息)**: 信息性消息
/// - **Debug (调试)**: 调试级别的消息
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// 系统不可用
    Emergency = 0,
    /// 必须立即采取行动
    Alert = 1,
    /// 关键状况
    Critical = 2,
    /// 错误状况
    Error = 3,
    /// 警告状况
    Warning = 4,
    /// 正常但重要的状况
    Notice = 5,
    /// 信息性消息
    Info = 6,
    /// 调试级别的消息
    Debug = 7,
}

impl LogLevel {
    /// 返回日志级别的字符串表示形式
    ///
    /// 返回一个简短的标签，如 `[ERR]`、`[INFO]` 等。
    pub(super) const fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Emergency => "[EMERG]",
            LogLevel::Alert => "[ALERT]",
            LogLevel::Critical => "[CRIT]",
            LogLevel::Error => "[ERR]",
            LogLevel::Warning => "[WARNING]",
            LogLevel::Notice => "[NOTICE]",
            LogLevel::Info => "[INFO]",
            LogLevel::Debug => "[DEBUG]",
        }
    }

    /// 返回此日志级别对应的 ANSI 颜色代码
    ///
    /// # 颜色映射
    ///
    /// - Emergency/Alert/Critical: 亮红色
    /// - Error: 红色
    /// - Warning: 黄色
    /// - Notice: 亮白色
    /// - Info: 白色
    /// - Debug: 灰色
    pub(super) const fn color_code(&self) -> &'static str {
        match self {
            Self::Emergency | Self::Alert | Self::Critical => "\x1b[1;31m",
            Self::Error => "\x1b[31m",
            Self::Warning => "\x1b[33m",
            Self::Notice => "\x1b[1;37m",
            Self::Info => "\x1b[37m",
            Self::Debug => "\x1b[90m",
        }
    }

    /// 返回 ANSI 颜色重置代码
    pub(super) const fn reset_color_code(&self) -> &'static str {
        "\x1b[0m"
    }

    /// 将 u8 值转换为日志级别
    ///
    /// 如果该值无效，则返回默认日志级别。
    pub fn from_u8(level: u8) -> Self {
        match level {
            0 => Self::Emergency,
            1 => Self::Alert,
            2 => Self::Critical,
            3 => Self::Error,
            4 => Self::Warning,
            5 => Self::Notice,
            6 => Self::Info,
            7 => Self::Debug,

            _ => super::config::DEFAULT_LOG_LEVEL,
        }
    }

    /// 将日志级别转换为 u8 值
    ///
    /// # 返回值
    /// 日志级别对应的数值 (0-7)
    pub const fn to_u8(self) -> u8 {
        self as u8
    }
}
