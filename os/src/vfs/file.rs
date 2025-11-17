use alloc::sync::Arc;
use crate::vfs::{FsError, InodeMetadata};

/// File trait - 会话层接口
///
/// 替代原有的 `struct File`,使用 trait 实现多态。
/// 这是进程文件描述符表中存储的核心接口 (Arc<dyn File>)。
///
/// # 设计原则
/// - read/write 方法不携带 offset 参数 (由实现者内部维护)
/// - 使用 Trait Objects 实现多态,而非 C 风格函数指针
/// - 支持可 seek 文件 (DiskFile) 和不可 seek 文件 (PipeFile, StdioFile)
pub trait File: Send + Sync {
    /// 检查文件是否可读
    fn readable(&self) -> bool;

    /// 检查文件是否可写
    fn writable(&self) -> bool;

    /// 从文件读取数据到缓冲区
    ///
    /// # 注意
    /// - 对于 DiskFile: 内部维护 offset,每次读取后自动更新
    /// - 对于 PipeFile: 从管道缓冲区读取,无 offset 概念
    /// - 对于 StdinFile: 从控制台读取,无 offset 概念
    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError>;

    /// 向文件写入数据
    fn write(&self, buf: &[u8]) -> Result<usize, FsError>;

    /// 获取文件元数据
    fn metadata(&self) -> Result<InodeMetadata, FsError>;

    /// 设置文件偏移量 (可选方法)
    ///
    /// 默认实现返回错误 (不支持 seek 的文件,如管道、stdin/stdout)
    ///
    /// # 参数
    /// - `offset`: 偏移量
    /// - `whence`: 起始位置 (SEEK_SET/SEEK_CUR/SEEK_END)
    ///
    /// # 返回
    /// - `Ok(new_offset)`: 新的文件偏移量
    /// - `Err(FsError::NotSupported)`: 文件不支持 seek
    fn lseek(&self, _offset: isize, _whence: SeekWhence) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }

    /// 获取当前偏移量 (可选方法,仅用于调试)
    ///
    /// 默认返回 0 (流式设备无 offset 概念)
    fn offset(&self) -> usize {
        0
    }

    /// 获取打开标志 (可选方法)
    ///
    /// 默认返回空标志 (流式设备如管道、stdio 可能没有 flags 概念)
    fn flags(&self) -> OpenFlags {
        OpenFlags::empty()
    }
}

/// lseek 的 whence 参数 
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum SeekWhence {
    Set = 0, // SEEK_SET
    Cur = 1, // SEEK_CUR
    End = 2, // SEEK_END
}

impl SeekWhence {
    pub fn from_usize(value: usize) -> Option<Self> {
        match value {
            0 => Some(SeekWhence::Set),
            1 => Some(SeekWhence::Cur),
            2 => Some(SeekWhence::End),
            _ => None,
        }
    }
}


bitflags::bitflags! {
    /// 打开标志
    pub struct OpenFlags: u32 {
        const O_RDONLY    = 0o0;
        const O_WRONLY    = 0o1;
        const O_RDWR      = 0o2;
        const O_ACCMODE   = 0o3;
        const O_CREAT     = 0o100;
        const O_EXCL      = 0o200;
        const O_TRUNC     = 0o1000;
        const O_APPEND    = 0o2000;
        const O_NONBLOCK  = 0o4000;
        const O_DIRECTORY = 0o200000;
        const O_CLOEXEC   = 0o2000000;
    }
}

impl OpenFlags {
    pub fn readable(&self) -> bool {
        let mode = self.bits() & OpenFlags::O_ACCMODE.bits();
        mode == OpenFlags::O_RDONLY.bits() || mode == OpenFlags::O_RDWR.bits()
    }

    pub fn writable(&self) -> bool {
        let mode = self.bits() & OpenFlags::O_ACCMODE.bits();
        mode == OpenFlags::O_WRONLY.bits() || mode == OpenFlags::O_RDWR.bits()
    }
}