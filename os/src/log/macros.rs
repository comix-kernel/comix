//! 日志宏
//!
//! 该模块提供了 **Linux 内核风格的日志宏** (`pr_*`)，用于以不同优先级级别进行轻松日志记录。
//!
//! # 用法
//!
//! ```rust
//! use crate::pr_info;
//! use crate::pr_err;
//!
//! pr_info!("系统已初始化");
//! pr_err!("分配 {} 字节失败", size);
//! pr_warn!("内存使用率为 {}%", percent);
//! pr_debug!("变量 x = {}", x);
//! ```
//!
//! # 宏列表
//!
//! - `pr_emerg!` - 紧急级别（系统不可用）
//! - `pr_alert!` - 警报级别（需要立即采取行动）
//! - `pr_crit!` - 关键级别（关键状况）
//! - `pr_err!` - 错误级别（错误状况）
//! - `pr_warn!` - 警告级别（警告状况）
//! - `pr_notice!` - 通知级别（正常但重要）
//! - `pr_info!` - 信息级别（信息性消息）
//! - `pr_debug!` - 调试级别（调试消息）
//!
//! # 性能
//!
//! 所有宏都在**宏展开时检查全局日志级别**。如果某个日志级别被禁用，则永远不会评估格式字符串，这使得**禁用的日志开销基本上为零**。

/// 带有级别过滤的内部实现宏
///
/// 在调用日志记录实现之前，**检查日志级别是否启用**。
/// 这种早期检查避免了对禁用级别进行不必要的格式化字符串评估。
#[macro_export]
macro_rules! __log_impl_filtered {
    ($level:expr, $args:expr) => {
        if $crate::log::is_level_enabled($level) {
            $crate::log::log_impl($level, $args);
        }
    };
}

/// 以 **EMERGENCY (紧急)** 级别记录消息
///
/// 紧急日志表示系统不可用。这些日志始终会打印到控制台（如果控制台输出可用）并存储在缓冲区中。
///
/// # 示例
///
/// ```rust
/// pr_emerg!("内核恐慌: {}", reason);
/// pr_emerg!("系统中止");
/// ```
#[macro_export]
macro_rules! pr_emerg {
($($arg:tt)*) => {
$crate::__log_impl_filtered!(
$crate::log::LogLevel::Emergency,
format_args!($($arg)*)
)
}
}

/// 以 **ALERT (警报)** 级别记录消息
///
/// 警报日志表示必须立即采取行动。
///
/// # 示例
///
/// ```rust
/// pr_alert!("检测到关键硬件故障");
/// ```
#[macro_export]
macro_rules! pr_alert {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Alert,
            format_args!($($arg)*)
        )
    }
}

/// 以 **CRITICAL (关键)** 级别记录消息
///
/// 关键日志表示需要关注的关键状况。
///
/// # 示例
///
/// ```rust
/// pr_crit!("温度阈值已超出");
/// ```
#[macro_export]
macro_rules! pr_crit {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Critical,
            format_args!($($arg)*)
        )
    }
}

/// 以 **ERROR (错误)** 级别记录消息
///
/// 错误日志表示在操作过程中发生的错误状况。
///
/// # 示例
///
/// ```rust
/// pr_err!("分配 {} 字节失败", size);
/// pr_err!("设备初始化失败: {}", error);
/// ```
#[macro_export]
macro_rules! pr_err {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Error,
            format_args!($($arg)*)
        )
    }
}

/// 以 **WARNING (警告)** 级别记录消息
///
/// 警告日志表示应审查但不妨碍正常操作的状况。
///
/// # 示例
///
/// ```rust
/// pr_warn!("内存使用率为 {}%", percent);
/// pr_warn!("使用了已弃用的功能");
/// ```
#[macro_export]
macro_rules! pr_warn {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Warning,
            format_args!($($arg)*)
        )
    }
}

/// 以 **NOTICE (通知)** 级别记录消息
///
/// 通知日志表示正常但重要的状况。
///
/// # 示例
///
/// ```rust
/// pr_notice!("设备 {} 已连接", device_name);
/// ```
#[macro_export]
macro_rules! pr_notice {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Notice,
            format_args!($($arg)*)
        )
    }
}

/// 以 **INFO (信息)** 级别记录消息
///
/// 信息日志提供有关正常系统操作的信息性消息。
///
/// # 示例
///
/// ```rust
/// pr_info!("内核已初始化");
/// pr_info!("正在启动子系统 {}", name);
/// ```
#[macro_export]
macro_rules! pr_info {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Info,
            format_args!($($arg)*)
        )
    }
}

/// 以 **DEBUG (调试)** 级别记录消息
///
/// 调试日志提供详细的诊断信息，用于故障排除。
/// 这些日志通常在生产版本中被禁用。
///
/// # 示例
///
/// ```rust
/// pr_debug!("调用函数时 x = {}", x);
/// pr_debug!("状态转换: {} -> {}", old_state, new_state);
/// ```
#[macro_export]
macro_rules! pr_debug {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Debug,
            format_args!($($arg)*)
        )
    }
}
