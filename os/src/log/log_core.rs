//! 日志系统核心实现
//!
//! 该模块将所有日志状态和逻辑封装到一个单独的 `LogCore` 结构体中，
//! 可以在保持**无锁、零分配**设计的同时，独立实例化用于测试。

use crate::arch::lib::console::Stdout;

use super::buffer::GlobalLogBuffer;
use super::config::{DEFAULT_CONSOLE_LEVEL, DEFAULT_LOG_LEVEL};
use super::context;
use super::entry::LogEntry;
use super::level::LogLevel;
use core::fmt;
use core::sync::atomic::{AtomicU8, Ordering};

/// 核心日志系统
///
/// 封装了环形缓冲区和过滤状态。可以为测试目的而实例化，
/// 或在生产环境中用作全局单例。
///
/// # 线程安全性
///
/// 所有方法都使用原子操作进行同步，使得整个结构体在
/// 线程之间安全共享，无需外部加锁。
pub struct LogCore {
    /// 用于日志存储的无锁环形缓冲区
    buffer: GlobalLogBuffer,

    /// 全局日志级别阈值（控制日志是否缓冲）
    global_level: AtomicU8,

    /// 控制台输出级别阈值（控制是否立即打印）
    console_level: AtomicU8,
}

impl LogCore {
    /// 使用默认日志级别创建新的 LogCore 实例
    ///
    /// 这是一个 `const fn`，可以在编译时进行评估，
    /// 从而实现零开销的静态初始化。
    ///
    /// 使用配置中的默认级别：
    /// - 全局级别: Info (Debug 级别的日志将被过滤)
    /// - 控制台级别: Warning (只打印 Warning 和 Error 级别的日志)
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 全局单例 (编译时初始化)
    /// static GLOBAL_LOG: LogCore = LogCore::default();
    /// ```
    pub const fn default() -> Self {
        Self {
            buffer: GlobalLogBuffer::new(),
            global_level: AtomicU8::new(DEFAULT_LOG_LEVEL as u8),
            console_level: AtomicU8::new(DEFAULT_CONSOLE_LEVEL as u8),
        }
    }

    /// 使用自定义日志级别创建新的 LogCore 实例
    ///
    /// 此构造函数允许在创建时指定全局和控制台日志级别，
    /// 这对于测试尤其有用。
    ///
    /// # 参数
    ///
    /// * `global_level` - 日志被缓冲的最低级别
    /// * `console_level` - 日志被打印到控制台的最低级别
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 启用 Debug 级别的测试实例
    /// let test_log = LogCore::new(LogLevel::Debug, LogLevel::Warning);
    ///
    /// // 使用自定义级别的生产实例
    /// let log = LogCore::new(LogLevel::Info, LogLevel::Error);
    /// ```
    pub fn new(global_level: LogLevel, console_level: LogLevel) -> Self {
        Self {
            buffer: GlobalLogBuffer::new(),
            global_level: AtomicU8::new(global_level as u8),
            console_level: AtomicU8::new(console_level as u8),
        }
    }

    /// 核心日志记录实现
    ///
    /// 此方法由生产宏（通过 GLOBAL_LOG）和测试代码（通过本地实例）调用。
    ///
    /// # 无锁操作
    ///
    /// 1. 原子读取 global_level (Acquire)
    /// 2. 如果被过滤，则提前返回
    /// 3. 收集上下文 (时间戳、CPU ID、任务 ID)
    /// 4. 创建日志条目 (栈分配)
    /// 5. 原子缓冲区写入 (无锁)
    /// 6. 可选的控制台输出 (如果满足 console_level)
    ///
    /// # 参数
    ///
    /// * `level` - 日志级别 (Emergency 到 Debug)
    /// * `args` - 来自 `format_args!` 的格式化参数
    pub fn _log(&self, level: LogLevel, args: fmt::Arguments) {
        // 1. 早期过滤 (全局级别)
        if !self.is_level_enabled(level) {
            return;
        }

        // 2. 收集上下文
        let log_context = context::collect_context();

        // 3. 创建日志条目
        let entry = LogEntry::from_args(
            level,
            log_context.cpu_id,
            log_context.task_id,
            log_context.timestamp,
            args,
        );

        // 4. 写入缓冲区 (无锁)
        self.buffer.write(&entry);

        // 5. 可选的即时控制台输出
        if self.is_console_level(level) {
            self.direct_print_entry(&entry);
        }
    }

    /// 从缓冲区读取下一个日志条目
    ///
    /// 如果没有可用条目，则返回 `None`。这是一个**无锁**的
    /// 单消费者操作。
    pub fn _read_log(&self) -> Option<LogEntry> {
        self.buffer.read()
    }

    /// 非破坏性读取：按索引 peek 日志条目，不移动读指针
    pub fn _peek_log(&self, index: usize) -> Option<LogEntry> {
        self.buffer.peek(index)
    }

    /// 获取当前可读取的起始索引
    pub fn _log_reader_index(&self) -> usize {
        self.buffer.reader_index()
    }

    /// 获取当前写入位置
    pub fn _log_writer_index(&self) -> usize {
        self.buffer.writer_index()
    }

    /// 返回未读日志条目的数量
    pub fn _log_len(&self) -> usize {
        self.buffer.len()
    }

    /// 返回未读日志的总字节数（格式化后）
    pub fn _log_unread_bytes(&self) -> usize {
        self.buffer.unread_bytes()
    }

    /// 返回由于缓冲区溢出而丢弃的日志计数
    pub fn _log_dropped_count(&self) -> usize {
        self.buffer.dropped_count()
    }

    /// 设置全局日志级别阈值
    ///
    /// 级别 > 阈值的日志将被丢弃。
    ///
    /// # 内存顺序
    ///
    /// 使用 Release 顺序以确保新级别对所有核心可见。
    pub fn _set_global_level(&self, level: LogLevel) {
        self.global_level.store(level as u8, Ordering::Release);
    }

    /// 获取当前全局日志级别
    pub fn _get_global_level(&self) -> LogLevel {
        let level = self.global_level.load(Ordering::Acquire);
        LogLevel::from_u8(level)
    }

    /// 设置控制台输出级别阈值
    ///
    /// 只有级别 <= 阈值的日志才会立即打印。
    pub fn _set_console_level(&self, level: LogLevel) {
        self.console_level.store(level as u8, Ordering::Release);
    }

    /// 获取当前控制台输出级别
    pub fn _get_console_level(&self) -> LogLevel {
        let level = self.console_level.load(Ordering::Acquire);
        LogLevel::from_u8(level)
    }

    // ========== 内部辅助函数 ==========

    /// 检查日志级别是否启用 (全局过滤器)
    #[inline(always)]
    fn is_level_enabled(&self, level: LogLevel) -> bool {
        level as u8 <= self.global_level.load(Ordering::Acquire)
    }

    /// 检查日志是否应该打印到控制台
    #[inline(always)]
    fn is_console_level(&self, level: LogLevel) -> bool {
        level as u8 <= self.console_level.load(Ordering::Acquire)
    }

    /// 使用 ANSI 颜色直接将日志条目打印到控制台（无堆分配）
    ///
    /// 此方法在早期启动时即可使用，因为它仅使用栈和 core::fmt::Write，
    /// 不依赖堆分配器。
    ///
    /// **重要**: 此函数的格式化逻辑必须与 `format_log_entry` 和 `buffer::calculate_formatted_length` 保持一致。
    /// 如果修改了日志输出格式，需要同步更新三处：
    /// - `direct_print_entry` (此函数) - 用于早期启动的控制台输出
    /// - `format_log_entry` - 用于 syslog 系统调用
    /// - `buffer::calculate_formatted_length` - 用于精确字节计数
    fn direct_print_entry(&self, entry: &LogEntry) {
        use core::fmt::Write;

        let mut stdout = Stdout;
        // 直接格式化输出，不使用堆分配
        let _ = write!(
            stdout,
            "{}{} [{:12}] [CPU{}/T{:3}] {}{}",
            entry.level().color_code(),
            entry.level().as_str(),
            entry.timestamp(),
            entry.cpu_id(),
            entry.task_id(),
            entry.message(),
            entry.level().reset_color_code()
        );
        let _ = writeln!(stdout);
    }
}

// 标记为 Sync 允许在 static 中使用
unsafe impl Sync for LogCore {}

/// 格式化日志条目为字符串（带 ANSI 颜色和上下文信息）
///
/// 将 LogEntry 格式化为用户可读的字符串，用于 syslog 系统调用等场景。
/// 包含 ANSI 颜色代码、时间戳、CPU ID、任务 ID 等上下文信息。
///
/// **注意**：此函数使用堆分配（`alloc::format!`），仅在堆分配器初始化后可用。
/// 主要用于 syslog 系统调用等运行时场景。早期启动时的控制台输出使用
/// `direct_print_entry` 方法，该方法不依赖堆分配。
///
/// **重要**：此函数的格式化逻辑必须与 `direct_print_entry` 和 `buffer::calculate_formatted_length` 保持一致。
/// 如果修改了日志输出格式，需要同步更新三处：
/// - `direct_print_entry` - 用于早期启动的控制台输出（无堆分配）
/// - `format_log_entry` (此函数) - 用于 syslog 系统调用（使用堆分配）
/// - `buffer::calculate_formatted_length` - 用于精确字节计数
///
/// # 格式
/// ```
/// <color_code>[LEVEL] [timestamp] [CPU<id>/T<tid>] message<reset>
/// ```
///
/// # 示例
/// ```
/// \x1b[37m[INFO] [      123456] [CPU0/T  1] Kernel initialized\x1b[0m
/// \x1b[31m[ERR] [      789012] [CPU0/T  5] Failed to mount /dev/sda1\x1b[0m
/// ```
///
/// # 参数
/// * `entry` - 要格式化的日志条目
///
/// # 返回值
/// 格式化后的字符串（包含 ANSI 颜色代码和上下文信息）
pub fn format_log_entry(entry: &LogEntry) -> alloc::string::String {
    use alloc::format;

    format!(
        "{}{} [{:12}] [CPU{}/T{:3}] {}{}",
        entry.level().color_code(),
        entry.level().as_str(),
        entry.timestamp(),
        entry.cpu_id(),
        entry.task_id(),
        entry.message(),
        entry.level().reset_color_code()
    )
}
