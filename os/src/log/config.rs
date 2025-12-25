//! 日志系统配置常量
//!
//! 该模块定义了日志系统的**编译时配置参数**。

#![allow(unused)]

/// 全局日志缓冲区（以字节为单位）的总大小
///
/// 缓冲区实现为**固定大小的环形缓冲区**。当缓冲区满时，新日志将
/// 覆盖最旧的条目。对于一个 16KB 的缓冲区和典型的条目大小，
/// 大约可以存储 50-60 个日志条目。
pub const GLOBAL_LOG_BUFFER_SIZE: usize = 16 * 1024; // 16KB

/// 单个日志消息的最大长度（以字节为单位）
///
/// 超过此长度的消息将被**截断**。此限制可防止单个日志占用
/// 过多的缓冲区空间。
pub const MAX_LOG_MESSAGE_LENGTH: usize = 256;

/// 默认全局日志级别
///
/// 处于此级别或更高优先级的日志将被记录到缓冲区中。
/// 默认值为 `Info`，意味着 Debug 日志默认会被过滤掉。
pub const DEFAULT_LOG_LEVEL: super::level::LogLevel = super::level::LogLevel::Debug;

/// 默认控制台输出级别
///
/// 处于此级别或更高优先级的日志将**立即打印到控制台**。
/// 默认值为 `Warning`，意味着默认情况下只有警告和错误才会出现在控制台上。
pub const DEFAULT_CONSOLE_LEVEL: super::level::LogLevel = super::level::LogLevel::Debug;
