use super::level::LogLevel;
use core::fmt;
use core::cmp::min;

pub const MAX_LOG_MESSAGE_LENGTH: usize = 256;

#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct LogEntry {
    level: LogLevel,
    cpu_id: usize,
    length: usize,
    task_id: u32,
    timestamp: u64,
    message: [u8; MAX_LOG_MESSAGE_LENGTH],
}

impl LogEntry {
    pub const fn empty() -> Self {
        Self {
            level: LogLevel::Debug,
            cpu_id: 0,
            length: 0,
            task_id: 0,
            timestamp: 0,
            message: [0; MAX_LOG_MESSAGE_LENGTH],
        }
    }

    pub fn new(level: LogLevel, cpu_id: usize, task_id: u32, timestamp: u64, message: &str) -> Self {
        let bytes = message.as_bytes();
        let length = min(bytes.len(), MAX_LOG_MESSAGE_LENGTH);
        let mut message = [0; MAX_LOG_MESSAGE_LENGTH];
        message[..length].copy_from_slice(&bytes[..length]);
        Self {
            level,
            cpu_id,
            length,
            task_id,
            timestamp,
            message,
        }
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

    pub fn timestamp(&self) -> u64 {
        self.timestamp
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