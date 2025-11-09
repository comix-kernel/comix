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

    /// 返回未读日志条目的数量
    pub fn _log_len(&self) -> usize {
        self.buffer.len()
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

    /// 使用 ANSI 颜色直接将日志条目打印到控制台
    fn direct_print_entry(&self, entry: &LogEntry) {
        use core::fmt::Write;

        let mut stdout = Stdout;
        let _ = write!(
            stdout,
            "{}{} ",
            entry.level().color_code(),
            entry.level().as_str()
        );
        let _ = stdout.write_str(entry.message());
        let _ = write!(stdout, "{}", entry.level().reset_color_code());
        let _ = writeln!(stdout);
    }
}

// 标记为 Sync 允许在 static 中使用
unsafe impl Sync for LogCore {}
