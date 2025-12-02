//! 文件锁管理
//!
//! 实现 POSIX 文件锁（advisory locks）语义：
//! - 读锁（共享锁）之间兼容
//! - 写锁（独占锁）与任何锁互斥
//! - 同一进程的锁可以合并/覆盖
//! - 进程退出时自动释放所有锁

use crate::sync::SpinLock;
use crate::uapi::fcntl::{Flock, LockType};
use crate::vfs::FsError;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// 文件锁条目
#[derive(Debug, Clone)]
struct FileLockEntry {
    /// 锁类型
    lock_type: LockType,
    /// 起始位置（绝对偏移）
    start: usize,
    /// 长度
    len: usize,
    /// 持有锁的进程 PID
    pid: i32,
}

impl FileLockEntry {
    /// 检查锁范围是否重叠
    fn overlaps(&self, start: usize, len: usize) -> bool {
        let self_end = self.start.saturating_add(self.len);
        let other_end = start.saturating_add(len);
        !(self_end <= start || other_end <= self.start)
    }

    /// 检查与另一个锁是否冲突
    fn conflicts_with(&self, other: &FileLockEntry) -> bool {
        if !self.overlaps(other.start, other.len) {
            return false;
        }

        // 同一进程的锁不冲突
        if self.pid == other.pid {
            return false;
        }

        // 读锁之间不冲突
        if self.lock_type == LockType::Read && other.lock_type == LockType::Read {
            return false;
        }

        // 其他情况都冲突
        true
    }
}

/// 文件标识符（设备号 + inode 号）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct FileId {
    dev: u64,
    ino: u64,
}

/// 全局文件锁管理器
pub struct FileLockManager {
    /// 文件锁表：FileId -> 锁列表
    locks: SpinLock<BTreeMap<FileId, Vec<FileLockEntry>>>,
}

impl FileLockManager {
    /// 创建新的文件锁管理器
    pub const fn new() -> Self {
        Self {
            locks: SpinLock::new(BTreeMap::new()),
        }
    }

    /// 测试锁（F_GETLK）
    ///
    /// 检查是否有锁会阻塞请求的锁。如果有冲突，返回冲突锁的信息。
    pub fn test_lock(
        &self,
        dev: u64,
        ino: u64,
        start: usize,
        len: usize,
        flock: &mut Flock,
        pid: i32,
    ) -> Result<(), FsError> {
        let file_id = FileId { dev, ino };
        let locks = self.locks.lock();

        let lock_type = match LockType::from_raw(flock.l_type) {
            Some(LockType::Read) | Some(LockType::Write) => {
                LockType::from_raw(flock.l_type).unwrap()
            }
            _ => return Err(FsError::InvalidArgument),
        };

        // 构造请求的锁（使用传入的范围参数）
        let requested_lock = FileLockEntry {
            lock_type,
            start,
            len,
            pid,
        };

        // 检查是否有冲突的锁
        if let Some(file_locks) = locks.get(&file_id) {
            for existing_lock in file_locks {
                if existing_lock.conflicts_with(&requested_lock) {
                    // 找到冲突的锁，填充 flock 结构
                    flock.l_type = existing_lock.lock_type as i16;
                    flock.l_start = existing_lock.start as i64;
                    flock.l_len = existing_lock.len as i64;
                    flock.l_pid = existing_lock.pid;
                    flock.l_whence = 0; // SEEK_SET
                    return Ok(());
                }
            }
        }

        // 没有冲突，设置为 F_UNLCK
        flock.l_type = LockType::Unlock as i16;
        Ok(())
    }

    /// 设置锁（F_SETLK / F_SETLKW）
    ///
    /// # 参数
    /// - `blocking`: true 表示阻塞（F_SETLKW），false 表示非阻塞（F_SETLK）
    ///
    /// # TODO: 实现 F_SETLKW 阻塞等待
    /// 当前实现在锁冲突时立即返回 WouldBlock，即使 blocking=true。
    ///
    /// 完整的 F_SETLKW 实现需要：
    /// 1. 在 FileLockManager 中为每个文件维护一个 WaitQueue
    /// 2. 锁冲突时，如果 blocking=true：
    ///    - 将当前任务加入该文件的等待队列
    ///    - 调用 yield_task() 让出 CPU
    ///    - 被唤醒后重新检查并尝试获取锁（可能需要循环）
    /// 3. 释放锁时（包括进程退出），唤醒等待队列中的所有任务
    /// 4. 需要处理信号中断（返回 EINTR）
    ///
    /// 参考实现：
    /// ```ignore
    /// loop {
    ///     if can_acquire_lock() {
    ///         acquire_and_break();
    ///     }
    ///     if !blocking {
    ///         return Err(WouldBlock);
    ///     }
    ///     // 检查信号
    ///     if has_pending_signal() {
    ///         return Err(Interrupted);
    ///     }
    ///     wait_queue.sleep(current_task());
    /// }
    /// ```
    pub fn set_lock(
        &self,
        dev: u64,
        ino: u64,
        start: usize,
        len: usize,
        lock_type: LockType,
        pid: i32,
        _blocking: bool,
    ) -> Result<(), FsError> {
        let file_id = FileId { dev, ino };
        let mut locks = self.locks.lock();

        match lock_type {
            LockType::Unlock => {
                // 释放锁：移除指定范围的锁
                if let Some(file_locks) = locks.get_mut(&file_id) {
                    file_locks.retain(|lock| !(lock.pid == pid && lock.overlaps(start, len)));
                    if file_locks.is_empty() {
                        locks.remove(&file_id);
                    }
                }
                Ok(())
            }
            LockType::Read | LockType::Write => {
                // 检查是否有冲突
                let file_locks = locks.entry(file_id).or_insert_with(Vec::new);

                let new_lock = FileLockEntry {
                    lock_type,
                    start,
                    len,
                    pid,
                };

                // 检查冲突
                for existing_lock in file_locks.iter() {
                    if existing_lock.conflicts_with(&new_lock) {
                        // 有冲突
                        // TODO: 如果 blocking=true，应该阻塞等待
                        return Err(FsError::WouldBlock);
                    }
                }

                // 移除同一进程在重叠范围内的旧锁
                file_locks.retain(|lock| !(lock.pid == pid && lock.overlaps(start, len)));

                // 添加新锁
                file_locks.push(new_lock);
                Ok(())
            }
        }
    }

    /// 释放进程持有的所有锁（进程退出时调用）
    pub fn release_all_locks(&self, pid: i32) {
        let mut locks = self.locks.lock();
        for file_locks in locks.values_mut() {
            file_locks.retain(|lock| lock.pid != pid);
        }
        locks.retain(|_, file_locks| !file_locks.is_empty());
    }
}

/// 全局文件锁管理器实例
static FILE_LOCK_MANAGER: FileLockManager = FileLockManager::new();

/// 获取全局文件锁管理器
pub fn file_lock_manager() -> &'static FileLockManager {
    &FILE_LOCK_MANAGER
}
