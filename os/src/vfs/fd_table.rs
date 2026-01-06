//! 文件描述符表
//!
//! 该模块实现了进程级的文件描述符管理，提供 POSIX 兼容的文件描述符操作。
//!
//! # 核心组件
//!
//! - [`FDTable`] - 文件描述符表，每个进程维护一个实例
//! - [`FdFlags`] - 文件描述符标志（如 FD_CLOEXEC）
//!
//! # 设计理念
//!
//! ## 文件描述符的本质
//!
//! 文件描述符（FD）是进程级的整数索引，指向打开的文件对象：
//!
//! ```text
//! FDTable
//! ┌────┬──────────────────────┐
//! │ 0  │ Arc<StdinFile>       │ ← stdin
//! │ 1  │ Arc<StdoutFile>      │ ← stdout
//! │ 2  │ Arc<StderrFile>      │ ← stderr
//! │ 3  │ Arc<RegFile>         │
//! │ 4  │ Arc<PipeFile>        │
//! │ 5  │ None                 │
//! └────┴──────────────────────┘
//! ```
//!
//! ## POSIX 语义
//!
//! ### 最小可用 FD
//!
//! `alloc()` 总是返回最小的可用文件描述符：
//!
//! ```rust
//! let fd1 = fd_table.alloc(file1)?;  // 返回 3
//! let fd2 = fd_table.alloc(file2)?;  // 返回 4
//! fd_table.close(3)?;
//! let fd3 = fd_table.alloc(file3)?;  // 返回 3 (重用)
//! ```
//!
//! ### dup 语义
//!
//! `dup` 系列操作共享 File 对象（包括 offset）：
//!
//! ```text
//! fd[3] ──┐
//!         ├──> Arc<RegFile> { offset: AtomicUsize }
//! fd[4] ──┘
//!
//! // fd[3] 和 fd[4] 共享 offset，一个 read() 会影响另一个
//! ```
//!
//! ### FD 标志 vs 文件状态标志
//!
//! - **FD 标志** (FdFlags): 文件描述符级别，dup 时不共享
//!   - `FD_CLOEXEC`: exec 时关闭
//! - **文件状态标志** (OpenFlags): File 对象级别，dup 时共享
//!   - `O_RDONLY`/`O_WRONLY`/`O_RDWR`: 访问模式
//!   - `O_APPEND`: 追加模式
//!   - `O_NONBLOCK`: 非阻塞模式
//!
//! ## fork 和 exec
//!
//! ### fork 时
//!
//! 父子进程共享整个 FDTable：
//!
//! ```text
//! 父进程 ──┐
//!          ├──> Arc<FDTable>
//! 子进程 ──┘
//! ```
//!
//! ### exec 时
//!
//! 自动关闭带 `FD_CLOEXEC` 标志的文件：
//!
//! ```rust
//! fd_table.close_exec();  // exec 前调用
//! ```
//!
//! # 并发安全
//!
//! FDTable 内部使用 `SpinLock` 保护：
//! - `files`: `Vec<Option<Arc<dyn File>>>`
//! - `fd_flags`: `Vec<FdFlags>`
//!
//! 多线程访问 FDTable 时会竞争锁，但 File 对象本身（如 RegFile 的 offset）
//! 使用原子操作，无需额外锁定。
//!
//! # 使用示例
//!
//! ## 基本操作
//!
//! ```rust
//! use vfs::{FDTable, RegFile};
//!
//! let fd_table = FDTable::new();
//!
//! // 分配文件描述符
//! let file = Arc::new(RegFile::new(dentry, OpenFlags::O_RDONLY));
//! let fd = fd_table.alloc(file)?;
//!
//! // 访问文件
//! let file = fd_table.get(fd)?;
//! file.read(&mut buf)?;
//!
//! // 关闭文件描述符
//! fd_table.close(fd)?;
//! ```
//!
//! ## dup 操作
//!
//! ```rust
//! // dup: 复制到最小可用 FD
//! let new_fd = fd_table.dup(old_fd)?;
//!
//! // dup2: 复制到指定 FD
//! fd_table.dup2(old_fd, new_fd)?;
//!
//! // dup3: dup2 + 设置标志
//! fd_table.dup3(old_fd, new_fd, OpenFlags::O_CLOEXEC)?;
//! ```
//!
//! ## 重定向示例
//!
//! ```rust
//! // 将 stdout 重定向到文件
//! let file = open_file("/tmp/output.txt", O_WRONLY | O_CREAT)?;
//! let fd = fd_table.alloc(file)?;
//! fd_table.dup2(fd, 1)?;  // 1 = stdout
//! fd_table.close(fd)?;
//! // 现在所有 println! 都会写到文件
//! ```

use crate::config::DEFAULT_MAX_FDS;
use crate::sync::SpinLock;
use crate::uapi::fcntl::{FdFlags, OpenFlags};
use crate::vfs::{File, FsError};
use alloc::sync::Arc;
use alloc::vec::Vec;
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
    /// 默认值为 FdFlags::empty()
    fd_flags: SpinLock<Vec<FdFlags>>,

    /// 最大文件描述符数量
    max_fds: usize,
}

impl FdFlags {
    /// 从 OpenFlags 中提取 FD 标志（用于兼容性）
    ///
    /// `O_CLOEXEC` 在 open() 时可以指定，但本质上是 FD 标志。
    pub fn from_open_flags(flags: OpenFlags) -> Self {
        let mut fd_flags = FdFlags::empty();
        if flags.contains(OpenFlags::O_CLOEXEC) {
            fd_flags |= FdFlags::CLOEXEC;
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

    /// 取走并清空所有已打开的文件描述符。
    ///
    /// 返回的列表包含 (fd, file)；调用方 drop 这些 Arc 即可完成“关闭”语义。
    ///
    /// 说明：
    /// - 这不会做任何“按类型”的额外清理（例如 socket fd 的 (tid,fd)->handle 映射），
    ///   需要调用方在 drop 前自行处理。
    pub fn take_all(&self) -> Vec<(usize, Arc<dyn File>)> {
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();

        let mut out = Vec::new();
        for (fd, slot) in files.iter_mut().enumerate() {
            if let Some(file) = slot.take() {
                out.push((fd, file));
            }
        }
        for f in fd_flags.iter_mut() {
            *f = FdFlags::empty();
        }
        out
    }

    /// 分配一个新的文件描述符（默认无 FD 标志）
    pub fn alloc(&self, file: Arc<dyn File>) -> Result<usize, FsError> {
        self.alloc_with_flags(file, FdFlags::empty())
    }

    /// 分配一个新的文件描述符并指定 FD 标志
    pub fn alloc_with_flags(&self, file: Arc<dyn File>, flags: FdFlags) -> Result<usize, FsError> {
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
        self.install_at_with_flags(fd, file, FdFlags::empty())
    }

    /// 在指定的 FD 位置安装文件并指定 FD 标志
    pub fn install_at_with_flags(
        &self,
        fd: usize,
        file: Arc<dyn File>,
        flags: FdFlags,
    ) -> Result<(), FsError> {
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();

        if fd >= self.max_fds {
            return Err(FsError::InvalidArgument);
        }

        // 扩展数组到指定大小
        while files.len() <= fd {
            files.push(None);
            fd_flags.push(FdFlags::empty());
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
        fd_flags[fd] = FdFlags::empty();
        Ok(())
    }

    /// 复制文件描述符
    ///
    /// 返回新的 fd，与 old_fd 指向同一个 `Arc<dyn File>` (共享 offset)。
    pub fn dup(&self, old_fd: usize) -> Result<usize, FsError> {
        let file = self.get(old_fd)?;
        self.alloc(file)
    }

    /// 复制文件描述符，新 fd >= min_fd（F_DUPFD 语义）
    ///
    /// 返回新的 fd，与 old_fd 指向同一个 `Arc<dyn File>` (共享 offset)。
    /// 新分配的 fd 是 >= min_fd 的最小未使用文件描述符。
    pub fn dup_from(&self, old_fd: usize, min_fd: usize, flags: FdFlags) -> Result<usize, FsError> {
        let file = self.get(old_fd)?;
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();

        // 1. 先确保数组至少有 min_fd 个元素
        while files.len() < min_fd {
            files.push(None);
            fd_flags.push(FdFlags::empty());
        }

        // 2. 从 min_fd 开始查找最小可用 FD
        for (fd, slot) in files.iter_mut().enumerate().skip(min_fd) {
            if slot.is_none() {
                *slot = Some(file);
                fd_flags[fd] = flags;
                return Ok(fd);
            }
        }

        // 3. 如果没有空闲槽位，在数组末尾分配新的 fd
        let fd = files.len();
        if fd >= self.max_fds {
            return Err(FsError::TooManyOpenFiles);
        }

        files.push(Some(file));
        fd_flags.push(flags);
        Ok(fd)
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
        let fd_flags = FdFlags::from_open_flags(flags);

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
            if flags.contains(FdFlags::CLOEXEC) {
                *slot = None;
                *flags = FdFlags::empty();
            }
        }
    }

    /// 获取文件描述符标志 (F_GETFD)
    pub fn get_fd_flags(&self, fd: usize) -> Result<FdFlags, FsError> {
        let files = self.files.lock();
        let fd_flags = self.fd_flags.lock();

        if fd >= files.len() || files[fd].is_none() {
            return Err(FsError::BadFileDescriptor);
        }

        Ok(fd_flags[fd])
    }

    /// 设置文件描述符标志 (F_SETFD)
    pub fn set_fd_flags(&self, fd: usize, flags: FdFlags) -> Result<(), FsError> {
        let files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();

        if fd >= files.len() || files[fd].is_none() {
            return Err(FsError::BadFileDescriptor);
        }

        fd_flags[fd] = flags;
        Ok(())
    }
}
