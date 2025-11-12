use crate::config::DEFAULT_MAX_FDS;
use crate::sync::SpinLock;
use crate::vfs::{File, FsError};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;

/// 文件描述符表
///
/// 每个进程维护一个文件描述符表，管理打开的文件
pub struct FDTable {
    /// 文件描述符数组
    /// None 表示该 FD 未使用
    files: SpinLock<Vec<Option<Arc<File>>>>,

    /// 最大文件描述符数量
    max_fds: usize,
}

impl fmt::Debug for FDTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let files = self.files.lock();
        let used = files.iter().filter(|slot| slot.is_some()).count();
        f.debug_struct("FDTable")
            .field("max_fds", &self.max_fds)
            .field("slots", &files.len())
            .field("used", &used)
            .finish()
    }
}

impl FDTable {
    /// 创建新的文件描述符表
    pub fn new() -> Self {
        Self {
            files: SpinLock::new(Vec::new()),
            max_fds: DEFAULT_MAX_FDS,
        }
    }

    /// 分配一个新的文件描述符
    pub fn alloc(&self, file: Arc<File>) -> Result<usize, FsError> {
        let mut files = self.files.lock();

        // 查找最小可用 FD
        for (fd, slot) in files.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(file);
                return Ok(fd);
            }
        }

        // 如果没有空闲槽位，扩展数组
        let fd = files.len();
        if fd >= self.max_fds {
            return Err(FsError::TooManyOpenFiles);
        }

        files.push(Some(file));
        Ok(fd)
    }

    /// 在指定的 FD 位置安装文件
    pub fn install_at(&self, fd: usize, file: Arc<File>) -> Result<(), FsError> {
        let mut files = self.files.lock();

        if fd >= self.max_fds {
            return Err(FsError::InvalidArgument);
        }

        // 扩展数组到指定大小
        while files.len() <= fd {
            files.push(None);
        }

        // 替换（旧文件会自动通过 Arc 释放）
        files[fd] = Some(file);
        Ok(())
    }

    /// 获取文件对象
    pub fn get(&self, fd: usize) -> Result<Arc<File>, FsError> {
        let files = self.files.lock();
        files
            .get(fd)
            .and_then(|f| f.clone())
            .ok_or(FsError::BadFileDescriptor)
    }

    /// 关闭文件描述符
    pub fn close(&self, fd: usize) -> Result<(), FsError> {
        let mut files = self.files.lock();

        if fd >= files.len() || files[fd].is_none() {
            return Err(FsError::BadFileDescriptor);
        }

        files[fd] = None;
        Ok(())
    }

    /// dup
    pub fn dup(&self, old_fd: usize) -> Result<usize, FsError> {
        let file = self.get(old_fd)?;
        self.alloc(file)
    }

    /// dup2
    pub fn dup2(&self, old_fd: usize, new_fd: usize) -> Result<usize, FsError> {
        // 特殊情况：如果两个 FD 相同，直接返回
        if old_fd == new_fd {
            // 检查 old_fd 是否有效
            self.get(old_fd)?;
            return Ok(new_fd);
        }

        let file = self.get(old_fd)?;

        // 如果 new_fd 已打开，先关闭它（忽略错误）
        let _ = self.close(new_fd);

        self.install_at(new_fd, file)?;
        Ok(new_fd)
    }

    /// 克隆整个文件描述符表（用于 fork）
    pub fn clone_table(&self) -> Self {
        let files = self.files.lock().clone();
        Self {
            files: SpinLock::new(files),
            max_fds: self.max_fds,
        }
    }

    /// 关闭所有带有 CLOEXEC 标志的文件（用于 exec）
    pub fn close_exec(&self) {
        let mut files = self.files.lock();
        for slot in files.iter_mut() {
            if let Some(file) = slot {
                if file.flags.contains(crate::vfs::OpenFlags::O_CLOEXEC) {
                    *slot = None;
                }
            }
        }
    }
}
