use super::config::MAX_LOG_MESSAGE_LENGTH;
use super::level::LogLevel;
use core::cmp::min;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicUsize, Ordering};

#[repr(C, align(8))]
#[derive(Debug)]
pub struct LogEntry {
    seq: AtomicUsize,
    level: LogLevel,
    cpu_id: usize,
    length: usize,
    task_id: u32,
    timestamp: usize,
    message: [u8; MAX_LOG_MESSAGE_LENGTH],
}

impl LogEntry {
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

    pub fn new(
        level: LogLevel,
        cpu_id: usize,
        task_id: u32,
        timestamp: usize,
        message: &str,
    ) -> Self {
        let bytes = message.as_bytes();
        let length = min(bytes.len(), MAX_LOG_MESSAGE_LENGTH);
        let mut message = [0; MAX_LOG_MESSAGE_LENGTH];
        message[..length].copy_from_slice(&bytes[..length]);
        Self {
            seq: AtomicUsize::new(0),
            level,
            cpu_id,
            length,
            task_id,
            timestamp,
            message,
        }
    }

    pub fn from_args(
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

        let mut writer = MessageWriter::new(&mut entry.message);
        let _ = core::fmt::write(&mut writer, args);

        entry.length = writer.len();

        entry
    }

    pub fn message(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.message[..self.length]) }
    }

    pub fn level(&self) -> LogLevel {
        self.level
    }

    pub fn cpu_id(&self) -> usize {
        self.cpu_id
    }

    pub fn task_id(&self) -> u32 {
        self.task_id
    }

    pub fn timestamp(&self) -> usize {
        self.timestamp
    }
}

impl LogEntry {
    /// (内部使用) 复制数据到缓冲区槽位
    /// `dest` 是指向 buffer[slot] 的裸指针
    pub(super) unsafe fn copy_data_to(&self, dest: *mut LogEntry) {
        // 我们不能用 ptr::write, 因为它会覆盖 dest.seq
        // 我们必须逐个字段复制 *除了* seq
        (*dest).level = self.level;
        (*dest).cpu_id = self.cpu_id;
        (*dest).length = self.length;
        (*dest).task_id = self.task_id;
        (*dest).timestamp = self.timestamp;
        (*dest).message.copy_from_slice(&self.message);
    }

    /// (内部使用) 设置序列号并“发布”
    /// `dest` 是指向 buffer[slot] 的裸指针
    pub(super) unsafe fn publish(&self, dest: *mut LogEntry, seq_num: usize) {
        // 使用 Release 内存序, 确保所有上面的数据写入
        // 在 'seq' 更新之前对其他核心可见
        (*dest).seq.store(seq_num, Ordering::Release);
    }

    /// (内部使用) 检查槽位是否已准备好
    /// `slot_ptr` 是指向 buffer[slot] 的裸指针
    pub(super) unsafe fn is_ready(&self, slot_ptr: *const LogEntry, expected_seq: usize) -> bool {
        // 使用 Acquire 内存序, 与生产者的 'publish' (Release store) 配对
        (*slot_ptr).seq.load(Ordering::Acquire) == expected_seq
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

/// a helper to write message from args to [u8; MAX_LOG_MESSAGE_LENGTH]
struct MessageWriter<'a> {
    buffer: &'a mut [u8],
    pos: usize,
}

impl<'a> MessageWriter<'a> {
    fn new(buffer: &'a mut [u8]) -> Self {
        Self { buffer, pos: 0 }
    }

    fn len(&self) -> usize {
        self.pos
    }
}

impl<'a> Write for MessageWriter<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let remaining = self.buffer.get_mut(self.pos..).unwrap_or(&mut []);
        let to_copy = min(bytes.len(), remaining.len());

        remaining[..to_copy].copy_from_slice(&bytes[..to_copy]);
        self.pos += to_copy;
        Ok(())
    }
}
