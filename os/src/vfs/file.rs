use crate::sync::SpinLock;
use crate::vfs::{Dentry, FsError, Inode, InodeMetadata};
use alloc::sync::Arc;

/// 文件打开标志（与 POSIX 兼容）
bitflags::bitflags! {
    pub struct OpenFlags: u32 {
        // 访问模式（互斥）
        const O_RDONLY    = 0o0;        // 只读
        const O_WRONLY    = 0o1;        // 只写
        const O_RDWR      = 0o2;        // 读写
        const O_ACCMODE   = 0o3;        // 访问模式掩码

        // 文件创建标志
        const O_CREAT     = 0o100;      // 不存在则创建
        const O_EXCL      = 0o200;      // 与 O_CREAT 配合，文件必须不存在
        const O_TRUNC     = 0o1000;     // 截断文件到 0
        const O_APPEND    = 0o2000;     // 追加模式

        // 行为标志
        const O_NONBLOCK  = 0o4000;     // 非阻塞模式
        const O_DIRECTORY = 0o200000;   // 必须是目录
        const O_CLOEXEC   = 0o2000000;  // exec 时关闭
    }
}

impl OpenFlags {
    /// 检查是否可读
    pub fn readable(&self) -> bool {
        let mode = self.bits() & OpenFlags::O_ACCMODE.bits();
        mode == OpenFlags::O_RDONLY.bits() || mode == OpenFlags::O_RDWR.bits()
    }

    /// 检查是否可写
    pub fn writable(&self) -> bool {
        let mode = self.bits() & OpenFlags::O_ACCMODE.bits();
        mode == OpenFlags::O_WRONLY.bits() || mode == OpenFlags::O_RDWR.bits()
    }
}

/// lseek 的 whence 参数
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum SeekWhence {
    Set = 0, // SEEK_SET: 从文件开头
    Cur = 1, // SEEK_CUR: 从当前位置
    End = 2, // SEEK_END: 从文件末尾
}

impl SeekWhence {
    /// 从 usize 转换
    pub fn from_usize(value: usize) -> Option<Self> {
        match value {
            0 => Some(SeekWhence::Set),
            1 => Some(SeekWhence::Cur),
            2 => Some(SeekWhence::End),
            _ => None,
        }
    }
}

/// 打开的文件对象
pub struct File {
    /// 关联的 dentry
    pub dentry: Arc<Dentry>,

    /// 关联的 inode（缓存）
    pub inode: Arc<dyn Inode>,

    /// 当前文件偏移量
    offset: SpinLock<usize>,

    /// 打开标志
    pub flags: OpenFlags,
}

impl File {
    /// 创建新的文件对象
    pub fn new(dentry: Arc<Dentry>, flags: OpenFlags) -> Self {
        let inode = dentry.inode.clone();
        Self {
            dentry,
            inode,
            offset: SpinLock::new(0),
            flags,
        }
    }

    /// 读取文件
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        // 检查权限
        if !self.flags.readable() {
            return Err(FsError::PermissionDenied);
        }

        // 读取数据
        let mut offset = self.offset.lock();
        let n = self.inode.read_at(*offset, buf)?;

        // 更新偏移量
        *offset += n;

        Ok(n)
    }

    /// 写入文件
    pub fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        // 检查权限
        if !self.flags.writable() {
            return Err(FsError::PermissionDenied);
        }

        let mut offset = self.offset.lock();

        // 如果是追加模式，移动到文件末尾
        if self.flags.contains(OpenFlags::O_APPEND) {
            let meta = self.inode.metadata()?;
            *offset = meta.size;
        }

        // 写入数据
        let n = self.inode.write_at(*offset, buf)?;

        // 更新偏移量
        *offset += n;

        Ok(n)
    }

    /// 改变文件偏移量
    pub fn lseek(&self, offset: isize, whence: SeekWhence) -> Result<usize, FsError> {
        let mut pos = self.offset.lock();
        let meta = self.inode.metadata()?;

        // 计算新位置
        let new_pos = match whence {
            SeekWhence::Set => {
                if offset < 0 {
                    return Err(FsError::InvalidArgument);
                }
                offset as usize
            }
            SeekWhence::Cur => {
                let current = *pos as isize;
                let result = current + offset;
                if result < 0 {
                    return Err(FsError::InvalidArgument);
                }
                result as usize
            }
            SeekWhence::End => {
                let end = meta.size as isize;
                let result = end + offset;
                if result < 0 {
                    return Err(FsError::InvalidArgument);
                }
                result as usize
            }
        };

        // 更新位置
        *pos = new_pos;
        Ok(new_pos)
    }

    /// 获取文件元数据
    pub fn stat(&self) -> Result<InodeMetadata, FsError> {
        self.inode.metadata()
    }

    /// 获取当前偏移量
    pub fn offset(&self) -> usize {
        *self.offset.lock()
    }
}
