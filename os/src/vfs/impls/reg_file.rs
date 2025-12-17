//! 普通文件（Regular File）的 File trait 实现

use crate::sync::SpinLock;
use crate::vfs::{Dentry, File, FsError, Inode, InodeMetadata, OpenFlags, SeekWhence};
use alloc::sync::Arc;

/// 普通文件的 File 实现
///
/// 对底层 Inode 的会话包装，维护：
/// - 当前文件偏移量（offset）
/// - 打开标志位（O_RDONLY/O_WRONLY/O_APPEND 等）
/// - 异步 I/O 所有者 PID
///
/// # 并发安全
///
/// `offset`、`flags` 和 `owner` 使用 `SpinLock` 保护，因为多线程可能通过 `fork()` 共享同一个 fd。
pub struct RegFile {
    /// 关联的 dentry (保留,用于某些操作如 fstat)
    pub dentry: Arc<Dentry>,

    /// 关联的 inode (缓存,避免每次从 dentry 获取)
    pub inode: Arc<dyn Inode>,

    /// 当前文件偏移量 (需要锁保护,因为多线程可能共享 fd)
    offset: SpinLock<usize>,

    /// 打开标志位 (需要锁保护以支持 F_SETFL)
    flags: SpinLock<OpenFlags>,

    /// 异步 I/O 所有者 PID (接收 SIGIO 信号的进程)
    owner: SpinLock<Option<i32>>,
}

impl RegFile {
    /// 创建新的 RegFile 实例
    pub fn new(dentry: Arc<Dentry>, flags: OpenFlags) -> Self {
        let inode = dentry.inode.clone();
        Self {
            dentry,
            inode,
            offset: SpinLock::new(0),
            flags: SpinLock::new(flags),
            owner: SpinLock::new(None),
        }
    }

    /// 获取底层 inode 引用 (用于某些系统调用)
    pub fn inode(&self) -> Arc<dyn Inode> {
        self.inode.clone()
    }

    /// 获取底层 dentry 引用 (用于某些系统调用)
    pub fn dentry(&self) -> Arc<Dentry> {
        self.dentry.clone()
    }

    /// 设置文件状态标志 (F_SETFL)
    ///
    /// 只能修改部分标志，访问模式等不能被修改
    pub fn set_flags(&self, new_flags: OpenFlags) -> Result<(), FsError> {
        let mut flags = self.flags.lock();
        *flags = new_flags;
        Ok(())
    }
}

impl File for RegFile {
    fn readable(&self) -> bool {
        self.flags.lock().readable()
    }

    fn writable(&self) -> bool {
        self.flags.lock().writable()
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
        let flags = self.flags.lock();
        let write_offset = if flags.contains(OpenFlags::O_APPEND) {
            // O_APPEND: 总是写到文件末尾
            self.inode.metadata()?.size
        } else {
            *offset_guard
        };
        drop(flags); // 释放 flags 锁

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
        *self.flags.lock()
    }

    fn inode(&self) -> Result<Arc<dyn Inode>, FsError> {
        Ok(self.inode())
    }

    fn dentry(&self) -> Result<Arc<Dentry>, FsError> {
        Ok(self.dentry())
    }

    fn set_status_flags(&self, new_flags: OpenFlags) -> Result<(), FsError> {
        self.set_flags(new_flags)
    }

    fn get_owner(&self) -> Result<i32, FsError> {
        Ok(self.owner.lock().unwrap_or(0))
    }

    fn set_owner(&self, pid: i32) -> Result<(), FsError> {
        *self.owner.lock() = if pid == 0 { None } else { Some(pid) };
        Ok(())
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        self.inode.read_at(offset, buf)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        self.inode.write_at(offset, buf)
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
