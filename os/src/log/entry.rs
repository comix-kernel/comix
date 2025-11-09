//! 日志条目结构和序列化
//!
//! 该模块定义了表示单个日志消息及其元数据的 `LogEntry` 结构体，
//! 并提供了用于创建和格式化日志条目的实用程序。

use super::config::MAX_LOG_MESSAGE_LENGTH;
use super::level::LogLevel;
use core::cmp::min;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicUsize, Ordering};

/// 带有元数据和消息的单个日志条目
///
/// 该结构体经过精心布局，用于无锁同步：
/// - `seq` 字段用作生产者和消费者之间的**同步点**
/// - 带有 8 字节对齐的 C 表示形式确保了正确的**原子访问**
/// - 字段顺序经过优化以**最小化填充**
#[repr(C, align(8))]
#[derive(Debug)]
pub struct LogEntry {
    /// 用于同步的序列号（必须是第一个字段）
    seq: AtomicUsize,
    /// 日志级别 (Emergency, Error, Info, 等)
    level: LogLevel,
    /// 生成此日志的 CPU ID
    cpu_id: usize,
    /// 消息的实际长度（以字节为单位）
    length: usize,
    /// 生成此日志的任务/进程 ID
    task_id: u32,
    /// 创建日志时的时间戳
    timestamp: usize,
    /// 用于日志消息的固定大小缓冲区
    message: [u8; MAX_LOG_MESSAGE_LENGTH],
}

impl LogEntry {
    /// 创建一个空的日志条目（用于初始化）
    ///
    /// 这是一个 `const fn`，因此可以在编译时进行评估，
    /// 允许对全局缓冲区进行常数初始化。
    pub const fn empty() -> Self {
        Self {
            seq: AtomicUsize::new(0),
            level: LogLevel::Debug,
            cpu_id: 0,
            length: 0,
            task_id: 0,
            timestamp: 0,
            message: [0; MAX_LOG_MESSAGE_LENGTH],
        }
    }

    /// 从格式化参数创建日志条目
    ///
    /// # 参数
    ///
    /// * `level` - 日志级别
    /// * `cpu_id` - 生成日志的 CPU ID
    /// * `task_id` - 生成日志的任务 ID
    /// * `timestamp` - 日志的时间戳
    /// * `args` - 来自 `format_args!` 宏的格式化参数
    pub(super) fn from_args(
        level: LogLevel,
        cpu_id: usize,
        task_id: u32,
        timestamp: usize,
        args: fmt::Arguments,
    ) -> Self {
        let mut entry = Self {
            seq: AtomicUsize::new(0),
            level,
            cpu_id,
            length: 0,
            task_id,
            timestamp,
            message: [0; MAX_LOG_MESSAGE_LENGTH],
        };

        // 将消息格式化到固定大小的缓冲区中
        let mut writer = MessageWriter::new(&mut entry.message);
        let _ = core::fmt::write(&mut writer, args);

        entry.length = writer.len();

        entry
    }

    /// 将日志消息作为字符串切片返回
    pub fn message(&self) -> &str {
        // Safety: MessageWriter 确保了有效的 UTF-8
        unsafe { core::str::from_utf8_unchecked(&self.message[..self.length]) }
    }

    /// 返回日志级别
    pub fn level(&self) -> LogLevel {
        self.level
    }

    /// 返回生成此日志的 CPU ID
    pub fn cpu_id(&self) -> usize {
        self.cpu_id
    }

    /// 返回生成此日志的任务 ID
    pub fn task_id(&self) -> u32 {
        self.task_id
    }

    /// 返回此日志的时间戳
    pub fn timestamp(&self) -> usize {
        self.timestamp
    }
}

impl LogEntry {
    /// 将日志数据复制到缓冲区槽中（供内部使用）
    ///
    /// 复制除 `seq` 字段外的所有字段，`seq` 字段必须
    /// 通过 `publish()` 单独设置，以确保正确的内存顺序。
    ///
    /// # 安全性
    ///
    /// `dest` 必须指向环形缓冲区中有效的 `LogEntry`
    pub(super) unsafe fn copy_data_to(&self, dest: *mut LogEntry) {
        // 我们不能使用 ptr::write，因为它会覆盖 dest.seq
        // 我们必须逐个字段复制，**除了** seq
        unsafe {
            (*dest).level = self.level;
            (*dest).cpu_id = self.cpu_id;
            (*dest).length = self.length;
            (*dest).task_id = self.task_id;
            (*dest).timestamp = self.timestamp;
            (*dest).message.copy_from_slice(&self.message);
        }
    }

    /// 通过设置其序列号来发布条目（供内部使用）
    ///
    /// 使用 **Release** 内存顺序，以确保在序列号更新之前，
    /// 所有数据写入对其他核心都是可见的。
    ///
    /// # 安全性
    ///
    /// `dest` 必须指向环形缓冲区中有效的 `LogEntry`
    pub(super) unsafe fn publish(&self, dest: *mut LogEntry, seq_num: usize) {
        // 使用 Release 内存顺序，以确保在 'seq' 更新之前
        // 所有数据写入都是可见的
        unsafe {
            (*dest).seq.store(seq_num, Ordering::Release);
        }
    }

    /// 检查槽是否已准备好读取（供内部使用）
    ///
    /// 使用 **Acquire** 内存顺序与生产者在 `publish()` 中的 Release 存储配对，
    /// 确保正确的同步。
    ///
    /// # 安全性
    ///
    /// `slot_ptr` 必须指向环形缓冲区中有效的 `LogEntry`
    pub(super) unsafe fn is_ready(&self, slot_ptr: *const LogEntry, expected_seq: usize) -> bool {
        // 使用 Acquire 内存顺序与生产者的 Release 存储配对
        unsafe { (*slot_ptr).seq.load(Ordering::Acquire) == expected_seq }
    }
}

impl Clone for LogEntry {
    fn clone(&self) -> Self {
        Self {
            seq: AtomicUsize::new(self.seq.load(Ordering::Relaxed)),
            level: self.level,
            cpu_id: self.cpu_id,
            length: self.length,
            task_id: self.task_id,
            timestamp: self.timestamp,
            message: self.message,
        }
    }
}

impl fmt::Display for LogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:12}] [{}] [CPU{}/T{:3}] {}",
            self.timestamp,
            self.level.as_str(),
            self.cpu_id,
            self.task_id,
            self.message()
        )
    }
}

/// 辅助结构体，用于将格式化输出写入固定大小的字节缓冲区
///
/// 实现了 `core::fmt::Write`，用于在没有动态分配的情况下捕获来自 `format_args!` 的格式化输出。
/// 消息如果超出缓冲区大小则会被截断。
struct MessageWriter<'a> {
    buffer: &'a mut [u8],
    pos: usize,
}

impl<'a> MessageWriter<'a> {
    /// 使用给定缓冲区创建新的消息写入器
    fn new(buffer: &'a mut [u8]) -> Self {
        Self { buffer, pos: 0 }
    }

    /// 返回到目前为止写入的字节数
    fn len(&self) -> usize {
        self.pos
    }
}

impl Write for MessageWriter<'_> {
    /// 将字符串切片写入缓冲区，必要时截断
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let remaining = self.buffer.get_mut(self.pos..).unwrap_or(&mut []);
        let to_copy = min(bytes.len(), remaining.len());

        remaining[..to_copy].copy_from_slice(&bytes[..to_copy]);
        self.pos += to_copy;
        Ok(())
    }
}
