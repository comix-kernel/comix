use crate::sync::SpinLock;
use crate::vfs::{Dentry, File, FsError, Inode, InodeMetadata, OpenFlags, SeekWhence};
use alloc::sync::Arc;

/// 基于 Inode 的磁盘文件实现
///
/// 替代原有的 `struct File`,专门用于基于磁盘 Inode 的文件。
///
/// # 职责
/// - 维护会话状态 (offset, flags)
/// - 将无状态的 read/write 调用转换为有状态的 inode.read_at/write_at
/// - 处理 O_APPEND 等特殊标志
pub struct DiskFile {
    /// 关联的 dentry (保留,用于某些操作如 fstat)
    pub dentry: Arc<Dentry>,

    /// 关联的 inode (缓存,避免每次从 dentry 获取)
    pub inode: Arc<dyn Inode>,

    /// 当前文件偏移量 (需要锁保护,因为多线程可能共享 fd)
    offset: SpinLock<usize>,

    /// 打开标志位
    pub flags: OpenFlags,
}

impl DiskFile {
    /// 创建新的 DiskFile 实例
    pub fn new(dentry: Arc<Dentry>, flags: OpenFlags) -> Self {
        let inode = dentry.inode.clone();
        Self {
            dentry,
            inode,
            offset: SpinLock::new(0),
            flags,
        }
    }

    /// 获取底层 inode 引用 (用于某些系统调用)
    pub fn inode(&self) -> Arc<dyn Inode> {
        self.inode.clone()
    }
}

impl File for DiskFile {
    fn readable(&self) -> bool {
        self.flags.readable()
    }

    fn writable(&self) -> bool {
        self.flags.writable()
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        // 检查权限
        if !self.readable() {
            return Err(FsError::PermissionDenied);
        }

        // 获取当前偏移量
        let mut offset_guard = self.offset.lock();
        let current_offset = *offset_guard;

        // 调用 inode 的 read_at
        let nread = self.inode.read_at(current_offset, buf)?;

        // 更新偏移量
        *offset_guard = current_offset + nread;

        Ok(nread)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        // 检查权限
        if !self.writable() {
            return Err(FsError::PermissionDenied);
        }

        // 获取写入偏移量
        let mut offset_guard = self.offset.lock();
        let write_offset = if self.flags.contains(OpenFlags::O_APPEND) {
            // O_APPEND: 总是写到文件末尾
            self.inode.metadata()?.size
        } else {
            *offset_guard
        };

        // 调用 inode 的 write_at
        let nwritten = self.inode.write_at(write_offset, buf)?;

        // 更新偏移量
        *offset_guard = write_offset + nwritten;

        Ok(nwritten)
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        self.inode.metadata()
    }

    fn lseek(&self, offset: isize, whence: SeekWhence) -> Result<usize, FsError> {
        let mut offset_guard = self.offset.lock();
        let current = *offset_guard as isize;
        let file_size = self.inode.metadata()?.size as isize;

        let new_offset = match whence {
            SeekWhence::Set => offset,
            SeekWhence::Cur => current + offset,
            SeekWhence::End => file_size + offset,
        };

        // 检查偏移量合法性 (不能为负)
        if new_offset < 0 {
            return Err(FsError::InvalidArgument);
        }

        *offset_guard = new_offset as usize;
        Ok(new_offset as usize)
    }

    fn offset(&self) -> usize {
        *self.offset.lock()
    }

    fn flags(&self) -> OpenFlags {
        self.flags
    }
}
