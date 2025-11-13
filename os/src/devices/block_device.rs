/// 块设备 trait
///
/// 提供统一的块设备访问接口
pub trait BlockDevice: Send + Sync {
    /// 读取块
    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> Result<(), BlockError>;

    /// 写入块
    fn write_block(&self, block_id: usize, buf: &[u8]) -> Result<(), BlockError>;

    /// 获取块大小（字节）
    fn block_size(&self) -> usize;

    /// 获取总块数
    fn total_blocks(&self) -> usize;

    /// 同步缓冲区到磁盘
    fn flush(&self) -> Result<(), BlockError>;

    /// 获取设备标识符
    fn device_id(&self) -> usize;
}

/// 块设备错误
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockError {
    /// I/O 错误
    IoError,

    /// 无效的块号
    InvalidBlock,

    /// 设备未就绪
    DeviceNotReady,

    /// 写保护
    WriteProtected,

    /// 不支持的操作
    NotSupported,
}

impl BlockError {
    /// 转换为 FsError
    pub fn to_fs_error(&self) -> crate::vfs::FsError {
        use crate::vfs::FsError;
        match self {
            BlockError::IoError => FsError::IoError,
            BlockError::InvalidBlock => FsError::InvalidArgument,
            BlockError::DeviceNotReady => FsError::IoError,
            BlockError::WriteProtected => FsError::ReadOnlyFs,
            BlockError::NotSupported => FsError::NotSupported,
        }
    }
}