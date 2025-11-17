//! 管道文件实现
//!
//! 管道是流式单向通信设备，读端和写端分别由两个 [`PipeFile`] 实例表示。

use crate::sync::SpinLock;
use crate::vfs::{File, FileMode, FsError, InodeMetadata, InodeType, TimeSpec};
use alloc::collections::VecDeque;
use alloc::sync::Arc;

/// 管道环形缓冲区
///
/// 容量默认 4KB（POSIX 最小 512 字节）。
struct PipeRingBuffer {
    /// 内部缓冲区
    buffer: VecDeque<u8>,
    /// 缓冲区容量
    capacity: usize,
    /// 写端引用计数 (用于检测写端关闭)
    write_end_count: usize,
    /// 读端引用计数 (用于检测读端关闭)
    read_end_count: usize,
}

impl PipeRingBuffer {
    const DEFAULT_CAPACITY: usize = 4096; // POSIX 规定最小 512 字节

    fn new() -> Self {
        Self {
            buffer: VecDeque::with_capacity(Self::DEFAULT_CAPACITY),
            capacity: Self::DEFAULT_CAPACITY,
            write_end_count: 0,
            read_end_count: 0,
        }
    }

    /// 读取数据 (非阻塞)
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, FsError> {
        if self.buffer.is_empty() {
            // 写端已关闭且缓冲区为空 -> EOF
            if self.write_end_count == 0 {
                return Ok(0);
            }
            // 写端未关闭但缓冲区为空 -> 暂时返回0 (TODO: 配合调度器实现阻塞)
            return Ok(0);
        }

        let nread = buf.len().min(self.buffer.len());
        for i in 0..nread {
            buf[i] = self.buffer.pop_front().unwrap();
        }

        Ok(nread)
    }

    /// 写入数据 (非阻塞)
    fn write(&mut self, buf: &[u8]) -> Result<usize, FsError> {
        // 读端已关闭 -> EPIPE (应发送 SIGPIPE 信号)
        if self.read_end_count == 0 {
            return Err(FsError::PermissionDenied); // 暂用 PermissionDenied 代替 EPIPE
        }

        // 缓冲区已满 -> 暂时只写入可用空间 (TODO: 阻塞等待)
        let available = self.capacity - self.buffer.len();
        if available == 0 {
            return Ok(0);
        }

        let nwrite = buf.len().min(available);
        for &byte in &buf[..nwrite] {
            self.buffer.push_back(byte);
        }

        Ok(nwrite)
    }
}

/// 管道文件实现
///
/// 特点:
/// - 单向数据流 (读端和写端分别创建两个 PipeFile 实例)
/// - 流式设备 (无 offset 概念,不支持 lseek)
/// - 不依赖 Inode (纯内存结构)
pub struct PipeFile {
    /// 共享的环形缓冲区
    buffer: Arc<SpinLock<PipeRingBuffer>>,
    /// 文件端点类型
    end_type: PipeEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PipeEnd {
    Read,
    Write,
}

impl PipeFile {
    /// 创建管道对 (返回 [读端, 写端])
    ///
    /// # 示例
    /// ```rust
    /// let (pipe_read, pipe_write) = PipeFile::create_pair();
    /// fd_table.install_at(3, Arc::new(pipe_read) as Arc<dyn File>)?;
    /// fd_table.install_at(4, Arc::new(pipe_write) as Arc<dyn File>)?;
    /// ```
    pub fn create_pair() -> (Self, Self) {
        let buffer = Arc::new(SpinLock::new(PipeRingBuffer::new()));

        // 初始化引用计数
        {
            let mut buf = buffer.lock();
            buf.read_end_count = 1;
            buf.write_end_count = 1;
        }

        let read_end = Self {
            buffer: buffer.clone(),
            end_type: PipeEnd::Read,
        };

        let write_end = Self {
            buffer,
            end_type: PipeEnd::Write,
        };

        (read_end, write_end)
    }
}

impl File for PipeFile {
    fn readable(&self) -> bool {
        self.end_type == PipeEnd::Read
    }

    fn writable(&self) -> bool {
        self.end_type == PipeEnd::Write
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        if !self.readable() {
            return Err(FsError::InvalidArgument);
        }

        let mut ring_buf = self.buffer.lock();
        ring_buf.read(buf)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        if !self.writable() {
            return Err(FsError::InvalidArgument);
        }

        let mut ring_buf = self.buffer.lock();
        ring_buf.write(buf)
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        // 管道没有真实的 inode,返回虚拟元数据
        Ok(InodeMetadata {
            inode_no: 0,
            inode_type: InodeType::Fifo,
            size: 0,
            mode: FileMode::S_IFIFO | FileMode::S_IRUSR | FileMode::S_IWUSR,
            uid: 0,
            gid: 0,
            atime: TimeSpec::zero(),
            mtime: TimeSpec::zero(),
            ctime: TimeSpec::zero(),
            nlinks: 1,
            blocks: 0,
        })
    }

    // lseek 使用默认实现 (返回 NotSupported)
}

impl Drop for PipeFile {
    fn drop(&mut self) {
        // 减少引用计数
        let mut buf = self.buffer.lock();
        match self.end_type {
            PipeEnd::Read => buf.read_end_count -= 1,
            PipeEnd::Write => buf.write_end_count -= 1,
        }
    }
}
