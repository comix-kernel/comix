//! 环形缓冲区模块
//!
//! 该模块实现了一个通用的环形缓冲区数据结构，用于高效的循环数据存储和读取
const RING_BUFFER_SIZE: usize = 256;

/// 缓冲区状态枚举
#[derive(Copy, Clone, PartialEq)]
enum BufferStatus {
    FULL,
    EMPTY,
    NORMAL,
}

/// 环形缓冲区结构体
pub struct RingBuffer {
    arr: [u8; RING_BUFFER_SIZE],
    head: usize,
    tail: usize,
    status: BufferStatus,
}

impl RingBuffer {
    /// 创建一个新的环形缓冲区实例
    pub fn new() -> Self {
        RingBuffer {
            arr: [0; RING_BUFFER_SIZE],
            head: 0,
            tail: 0,
            status: BufferStatus::EMPTY,
        }
    }

    /// 从环形缓冲区读取一个字节
    pub fn read_byte(&mut self) -> Option<u8> {
        if self.status == BufferStatus::EMPTY {
            return None;
        }
        let byte = self.arr[self.tail];
        self.tail = (self.tail + 1) % RING_BUFFER_SIZE;
        if self.tail == self.head {
            self.status = BufferStatus::EMPTY;
        } else {
            self.status = BufferStatus::NORMAL;
        }
        Some(byte)
    }

    /// 向环形缓冲区写入一个字节
    pub fn write_byte(&mut self, byte: u8) -> Result<(), ()> {
        if self.status == BufferStatus::FULL {
            return Err(());
        }
        self.arr[self.head] = byte;
        self.head = (self.head + 1) % RING_BUFFER_SIZE;
        if self.head == self.tail {
            self.status = BufferStatus::FULL;
        } else {
            self.status = BufferStatus::NORMAL;
        }
        Ok(())
    }

    /// 获取环形缓冲区的可用空间
    pub fn available_space(&self) -> usize {
        match self.status {
            BufferStatus::FULL => 0,
            BufferStatus::EMPTY => RING_BUFFER_SIZE,
            BufferStatus::NORMAL => {
                if self.head >= self.tail {
                    RING_BUFFER_SIZE - (self.head - self.tail)
                } else {
                    self.tail - self.head
                }
            }
        }
    }
}
