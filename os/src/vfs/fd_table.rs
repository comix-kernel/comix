//! 文件描述符表
//!
//! 每个进程维护一个 [`FDTable`]，管理打开的文件。文件以 `Arc<dyn File>` 形式存储，
//! 支持异构文件类型（DiskFile、PipeFile、StdioFile等）。

use crate::config::DEFAULT_MAX_FDS;
use crate::sync::SpinLock;
use crate::vfs::{File, FsError, OpenFlags};
use alloc::sync::Arc;
use alloc::vec::Vec;
use bitflags::bitflags;
use core::fmt;

/// 文件描述符表
///
/// # 并发安全
///
/// 内部使用 `SpinLock` 保护，支持多线程访问。
pub struct FDTable {
    /// 文件描述符数组
    /// None 表示该 FD 未使用
    files: SpinLock<Vec<Option<Arc<dyn File>>>>,

    /// 文件描述符标志数组（与 files 索引对应）
    /// 默认值为 FDFlags::empty()
    fd_flags: SpinLock<Vec<FDFlags>>,

    /// 最大文件描述符数量
    max_fds: usize,
}

bitflags::bitflags! {
    /// 文件描述符标志（FD 级别的属性）
    ///
    /// 这些标志是每个文件描述符独立的属性，即使多个 FD 指向同一个文件对象，
    /// 它们也可以有不同的 FD 标志。
    ///
    /// 对应 POSIX 的 `fcntl(F_GETFD/F_SETFD)` 操作。
    pub struct FDFlags: u32 {
        /// exec 时关闭此文件描述符
        const FD_CLOEXEC = 1;
    }
}

impl FDFlags {
    /// 从 OpenFlags 中提取 FD 标志（用于兼容性）
    ///
    /// `O_CLOEXEC` 在 open() 时可以指定，但本质上是 FD 标志。
    pub fn from_open_flags(flags: OpenFlags) -> Self {
        let mut fd_flags = FDFlags::empty();
        if flags.contains(OpenFlags::O_CLOEXEC) {
            fd_flags |= FDFlags::FD_CLOEXEC;
        }
        fd_flags
    }
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
            fd_flags: SpinLock::new(Vec::new()),
            max_fds: DEFAULT_MAX_FDS,
        }
    }

    /// 分配一个新的文件描述符（默认无 FD 标志）
    pub fn alloc(&self, file: Arc<dyn File>) -> Result<usize, FsError> {
        self.alloc_with_flags(file, FDFlags::empty())
    }

    /// 分配一个新的文件描述符并指定 FD 标志
    pub fn alloc_with_flags(&self, file: Arc<dyn File>, flags: FDFlags) -> Result<usize, FsError> {
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();

        // 查找最小可用 FD
        for (fd, slot) in files.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(file);
                fd_flags[fd] = flags;
                return Ok(fd);
            }
        }

        // 如果没有空闲槽位，扩展数组
        let fd = files.len();
        if fd >= self.max_fds {
            return Err(FsError::TooManyOpenFiles);
        }

        files.push(Some(file));
        fd_flags.push(flags);
        Ok(fd)
    }

    /// 在指定的 FD 位置安装文件（默认无 FD 标志）
    pub fn install_at(&self, fd: usize, file: Arc<dyn File>) -> Result<(), FsError> {
        self.install_at_with_flags(fd, file, FDFlags::empty())
    }

    /// 在指定的 FD 位置安装文件并指定 FD 标志
    pub fn install_at_with_flags(
        &self,
        fd: usize,
        file: Arc<dyn File>,
        flags: FDFlags,
    ) -> Result<(), FsError> {
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();

        if fd >= self.max_fds {
            return Err(FsError::InvalidArgument);
        }

        // 扩展数组到指定大小
        while files.len() <= fd {
            files.push(None);
            fd_flags.push(FDFlags::empty());
        }

        // 替换（旧文件会自动通过 Arc 释放）
        files[fd] = Some(file);
        fd_flags[fd] = flags;
        Ok(())
    }

    /// 获取文件对象
    pub fn get(&self, fd: usize) -> Result<Arc<dyn File>, FsError> {
        let files = self.files.lock();
        files
            .get(fd)
            .and_then(|f| f.clone())
            .ok_or(FsError::BadFileDescriptor)
    }

    /// 关闭文件描述符
    pub fn close(&self, fd: usize) -> Result<(), FsError> {
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();

        if fd >= files.len() || files[fd].is_none() {
            return Err(FsError::BadFileDescriptor);
        }

        files[fd] = None;
        fd_flags[fd] = FDFlags::empty();
        Ok(())
    }

    /// 复制文件描述符
    ///
    /// 返回新的 fd，与 old_fd 指向同一个 `Arc<dyn File>` (共享 offset)。
    pub fn dup(&self, old_fd: usize) -> Result<usize, FsError> {
        let file = self.get(old_fd)?;
        self.alloc(file)
    }

    /// 复制文件描述符到指定位置
    ///
    /// 如果 new_fd 已打开，先关闭它。
    pub fn dup2(&self, old_fd: usize, new_fd: usize) -> Result<usize, FsError> {
        // 特殊情况：如果两个 FD 相同，直接返回
        if old_fd == new_fd {
            // 检查 old_fd 是否有效
            self.get(old_fd)?;
            return Ok(new_fd);
        }

        // 调用 dup3，不设置任何标志
        self.dup3(old_fd, new_fd, OpenFlags::empty())
    }

    /// 复制文件描述符到指定位置（dup3 语义）
    ///
    /// 如果 new_fd 已打开，先关闭它。
    /// 与 dup2 不同，dup3 不允许 old_fd == new_fd。
    ///
    /// # 参数
    /// - `flags`: 可以包含 `O_CLOEXEC`，用于设置新 FD 的 CLOEXEC 标志
    pub fn dup3(&self, old_fd: usize, new_fd: usize, flags: OpenFlags) -> Result<usize, FsError> {
        // dup3 不允许 old_fd == new_fd
        if old_fd == new_fd {
            return Err(FsError::InvalidArgument);
        }

        let file = self.get(old_fd)?;

        // 如果 new_fd 已打开，先关闭它（忽略错误）
        let _ = self.close(new_fd);

        // 提取 FD 标志
        let fd_flags = FDFlags::from_open_flags(flags);

        self.install_at_with_flags(new_fd, file, fd_flags)?;
        Ok(new_fd)
    }

    /// 克隆整个文件描述符表（用于 fork）
    ///
    /// 所有 `Arc<dyn File>` 引用计数递增，父子进程共享文件对象。
    /// FD 标志也会被复制。
    pub fn clone_table(&self) -> Self {
        let files = self.files.lock().clone();
        let fd_flags = self.fd_flags.lock().clone();
        Self {
            files: SpinLock::new(files),
            fd_flags: SpinLock::new(fd_flags),
            max_fds: self.max_fds,
        }
    }

    /// 关闭所有带有 CLOEXEC 标志的文件（用于 exec）
    pub fn close_exec(&self) {
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();

        for (slot, flags) in files.iter_mut().zip(fd_flags.iter_mut()) {
            if flags.contains(FDFlags::FD_CLOEXEC) {
                *slot = None;
                *flags = FDFlags::empty();
            }
        }
    }
}
