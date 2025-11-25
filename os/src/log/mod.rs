//! 内核日志子系统
//!
//! 该模块提供了一个类似 **Linux 内核风格的日志系统**，并在裸机环境中实现了**无锁环形缓冲区**。
//!
//! # 组件
//!
//! - [`buffer`] - 用于日志存储的无锁环形缓冲区
//! - [`config`] - 配置常量（缓冲区大小、消息长度限制）
//! - [`context`] - 上下文信息收集（CPU ID、任务 ID、时间戳）
//! - [`log_core`] - 核心日志实现 (LogCore)
//! - [`entry`] - 日志条目结构和序列化
//! - [`level`] - 日志级别定义（从 Emergency 到 Debug）
//! - [`macros`] - 面向用户的日志宏 (`pr_info!`, `pr_err!`, 等)
//!
//! # 设计概览
//!
//! ## 双输出策略
//!
//! 日志系统采用两层方法：
//!
//! 1. **即时控制台输出**：达到控制台级别阈值（默认：Warning 及以上）的日志会**直接打印到控制台**，以实现紧急可见性。
//! 2. **环形缓冲区存储**：所有达到全局级别阈值（默认：Info 及以上）的日志都会被写入**无锁环形缓冲区**，用于异步消费或事后分析。
//!
//! ## 性能特点
//!
//! - **无锁并发**：使用原子操作（fetch_add, CAS）而非互斥锁，支持多生产者日志记录而**不会阻塞**。
//! - **早期过滤**：日志级别检查在宏展开时发生，避免对禁用级别的日志进行格式化字符串评估。
//! - **固定大小分配**：**没有动态内存分配**；所有结构体使用编译时已知的大小，适用于裸机环境。
//! - **缓存优化**：读写器数据结构经过缓存行填充（64 字节），以防止多核系统上的**伪共享**。
//! - **尽可能零拷贝**：在可行的情况下，日志条目是**就地构造**的，以最大限度地减少内存操作。
//!
//! ## 架构特定集成
//!
//! 日志系统与架构特定组件集成：
//!
//! - **定时器**：通过 `arch::timer::get_time()` 收集时间戳
//! - **控制台**：通过 `console::Stdout` 输出（通常是 UART）
//! - **CPU ID**：通过 `arch::kernel::cpu::cpu_id()` 获取当前 CPU ID
//! - **任务 ID**：通过 `kernel::cpu::current_cpu()` 获取当前任务的 tid（若无任务则为 0）
//!
//! # 使用示例
//!
//! ```rust
//! use crate::log::*;
//!
//! // 基本日志记录
//! pr_info!("内核已初始化");
//! pr_err!("分配 {} 字节失败", size);
//!
//! // 配置日志级别
//! set_global_level(LogLevel::Debug);  // 记录所有级别
//! set_console_level(LogLevel::Error); // 只打印错误及以上的级别
//!
//! // 读取缓冲的日志
//! while let Some(entry) = read_log() {
//!     // 处理日志条目
//! }
//! ```

#![allow(unused)]
mod buffer;
mod config;
mod context;
mod entry;
mod level;
mod log_core;
pub mod macros;

pub use config::{
    DEFAULT_CONSOLE_LEVEL, DEFAULT_LOG_LEVEL, GLOBAL_LOG_BUFFER_SIZE, MAX_LOG_MESSAGE_LENGTH,
};
pub use entry::LogEntry;
pub use level::LogLevel;
pub use log_core::format_log_entry;

// ========== 全局单例 ==========

/// 全局日志系统实例
///
/// 使用 const fn 在编译时初始化，零运行时开销。
/// 所有日志宏和公共 API 都委托给此实例。
static GLOBAL_LOG: log_core::LogCore = log_core::LogCore::default();

// ========== 公共 API (精简封装) ==========

/// 核心日志实现（由宏调用）
#[doc(hidden)]
pub fn log_impl(level: LogLevel, args: core::fmt::Arguments) {
    GLOBAL_LOG._log(level, args);
}

/// 检查日志级别是否启用（由宏调用）
#[doc(hidden)]
pub fn is_level_enabled(level: LogLevel) -> bool {
    level as u8 <= GLOBAL_LOG._get_global_level() as u8
}

/// 从缓冲区读取下一个日志条目
pub fn read_log() -> Option<LogEntry> {
    GLOBAL_LOG._read_log()
}

/// 非破坏性读取：按索引 peek 日志条目，不移动读指针
pub fn peek_log(index: usize) -> Option<LogEntry> {
    GLOBAL_LOG._peek_log(index)
}

/// 获取当前可读取的起始索引
pub fn log_reader_index() -> usize {
    GLOBAL_LOG._log_reader_index()
}

/// 获取当前写入位置
pub fn log_writer_index() -> usize {
    GLOBAL_LOG._log_writer_index()
}

/// 返回未读日志条目的数量
pub fn log_len() -> usize {
    GLOBAL_LOG._log_len()
}

/// 返回未读日志的总字节数（格式化后）
pub fn log_unread_bytes() -> usize {
    GLOBAL_LOG._log_unread_bytes()
}

/// 返回已丢弃日志的计数
pub fn log_dropped_count() -> usize {
    GLOBAL_LOG._log_dropped_count()
}

/// 设置全局日志级别阈值
pub fn set_global_level(level: LogLevel) {
    GLOBAL_LOG._set_global_level(level);
}

/// 获取当前全局日志级别
pub fn get_global_level() -> LogLevel {
    GLOBAL_LOG._get_global_level()
}

/// 设置控制台输出级别阈值
pub fn set_console_level(level: LogLevel) {
    GLOBAL_LOG._set_console_level(level);
}

/// 获取当前控制台输出级别
pub fn get_console_level() -> LogLevel {
    GLOBAL_LOG._get_console_level()
}

// ========== 测试模块 ==========
#[cfg(test)]
mod tests;
