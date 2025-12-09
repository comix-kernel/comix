//! fcntl 系统调用实现

use crate::arch::trap::SumGuard;
use crate::kernel::current_cpu;
use crate::uapi::errno::EINVAL;
use crate::uapi::fcntl::{FcntlCmd, FdFlags, FileStatusFlags, Flock, LockType};
use crate::vfs::{FsError, OpenFlags, file_lock_manager};
use alloc::sync::Arc;

/// fcntl - 文件描述符操作
///
/// # 参数
/// - `fd`: 文件描述符
/// - `cmd`: fcntl 命令
/// - `arg`: 命令参数（根据 cmd 的不同而不同）
///
/// # 返回值
/// 成功返回非负值，失败返回负 errno
pub fn fcntl(fd: usize, cmd_raw: i32, arg: usize) -> isize {
    // 解析命令
    let cmd = match FcntlCmd::from_raw(cmd_raw) {
        Some(c) => c,
        None => return -(EINVAL as isize),
    };

    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();

    match cmd {
        FcntlCmd::GetFd => {
            // F_GETFD: 获取文件描述符标志
            match task.lock().fd_table.get_fd_flags(fd) {
                Ok(flags) => flags.bits() as isize,
                Err(e) => e.to_errno(),
            }
        }

        FcntlCmd::SetFd => {
            // F_SETFD: 设置文件描述符标志
            let flags = match FdFlags::from_bits(arg as u32) {
                Some(f) => f,
                None => return -(EINVAL as isize),
            };
            match task.lock().fd_table.set_fd_flags(fd, flags) {
                Ok(()) => 0,
                Err(e) => e.to_errno(),
            }
        }

        FcntlCmd::GetFl => {
            // F_GETFL: 获取文件状态标志
            let file = match task.lock().fd_table.get(fd) {
                Ok(f) => f,
                Err(e) => return e.to_errno(),
            };
            file.flags().bits() as isize
        }

        FcntlCmd::SetFl => {
            // F_SETFL: 设置文件状态标志
            // 只能修改特定标志（APPEND, NONBLOCK, ASYNC, DIRECT, NOATIME）
            let new_flags_raw = arg as u32;
            let new_status_flags = match FileStatusFlags::from_bits(new_flags_raw) {
                Some(f) => f,
                None => return -(EINVAL as isize),
            };

            // 检查是否只修改允许修改的标志
            if !new_status_flags.is_modifiable() {
                return -(EINVAL as isize);
            }

            let file = match task.lock().fd_table.get(fd) {
                Ok(f) => f,
                Err(e) => return e.to_errno(),
            };

            // 获取当前标志
            let current_flags = file.flags();

            // 保留访问模式和其他不可修改的标志
            let access_mode = current_flags & OpenFlags::O_ACCMODE;
            let non_modifiable = current_flags
                & !(OpenFlags::O_APPEND
                    | OpenFlags::O_NONBLOCK
                    | OpenFlags::from_bits_truncate(FileStatusFlags::ASYNC.bits())
                    | OpenFlags::from_bits_truncate(FileStatusFlags::DIRECT.bits())
                    | OpenFlags::from_bits_truncate(FileStatusFlags::NOATIME.bits()));

            // 构建新的标志：保留访问模式 + 保留不可修改部分 + 新的可修改标志
            let final_flags = access_mode
                | non_modifiable
                | OpenFlags::from_bits_truncate(new_status_flags.bits());

            // 调用 File trait 的 set_status_flags 方法
            match file.set_status_flags(final_flags) {
                Ok(()) => 0,
                Err(e) => e.to_errno(),
            }
        }

        // 文件描述符复制
        FcntlCmd::DupFd => {
            // F_DUPFD: 复制文件描述符，新 fd >= arg
            fcntl_dupfd(&task, fd, arg, false)
        }

        FcntlCmd::DupFdCloexec => {
            // F_DUPFD_CLOEXEC: 复制文件描述符并设置 CLOEXEC
            fcntl_dupfd(&task, fd, arg, true)
        }

        // === 文件锁操作 ===
        FcntlCmd::GetLk => {
            // F_GETLK: 测试锁
            let flock_ptr = arg as *mut Flock;
            if flock_ptr.is_null() {
                return -(EINVAL as isize);
            }

            // 读取用户空间的 flock 结构
            let mut flock = {
                let _guard = SumGuard::new();
                unsafe { core::ptr::read(flock_ptr) }
            };

            // 获取文件对象
            let file = match task.lock().fd_table.get(fd) {
                Ok(f) => f,
                Err(e) => return e.to_errno(),
            };

            // 获取 inode 元数据（需要设备号和 inode 号）
            let inode = match file.inode() {
                Ok(i) => i,
                Err(_) => {
                    // 不支持锁的文件类型（如管道）
                    return FsError::InvalidArgument.to_errno();
                }
            };

            let metadata = match inode.metadata() {
                Ok(m) => m,
                Err(e) => return e.to_errno(),
            };

            // 获取当前进程 PID
            let pid = task.lock().pid as i32;

            // 将相对偏移转换为绝对偏移
            let file_offset = file.offset();
            let file_size = metadata.size;
            let (start, len) = match flock.to_absolute_range(file_offset, file_size) {
                Ok(range) => range,
                Err(_) => return -(EINVAL as isize),
            };

            // 测试锁
            // TODO: 获取真实设备号
            // 当前使用设备号 0，在单一文件系统场景下 inode 号足够区分文件。
            // 未来改进：
            // 1. 在 FileSystem trait 中添加 dev_id() 方法返回设备号
            // 2. 在挂载时为每个文件系统分配唯一的设备号
            // 3. 通过 dentry -> mount_point -> fs 获取设备号
            let dev = 0;
            let ino = metadata.inode_no as u64;
            if let Err(e) = file_lock_manager().test_lock(dev, ino, start, len, &mut flock, pid) {
                return e.to_errno();
            }

            // 将结果写回用户空间
            {
                let _guard = SumGuard::new();
                unsafe { core::ptr::write(flock_ptr, flock) };
            }

            0
        }

        FcntlCmd::SetLk | FcntlCmd::SetLkW => {
            // F_SETLK / F_SETLKW: 设置或释放锁
            let blocking = matches!(cmd, FcntlCmd::SetLkW);
            let flock_ptr = arg as *const Flock;
            if flock_ptr.is_null() {
                return -(EINVAL as isize);
            }

            // 读取用户空间的 flock 结构
            let flock = {
                let _guard = SumGuard::new();
                unsafe { core::ptr::read(flock_ptr) }
            };

            // 解析锁类型
            let lock_type = match LockType::from_raw(flock.l_type) {
                Some(t) => t,
                None => return -(EINVAL as isize),
            };

            // 获取文件对象
            let file = match task.lock().fd_table.get(fd) {
                Ok(f) => f,
                Err(e) => return e.to_errno(),
            };

            // 获取 inode
            let inode = match file.inode() {
                Ok(i) => i,
                Err(_) => {
                    // 不支持锁的文件类型
                    return FsError::InvalidArgument.to_errno();
                }
            };

            let metadata = match inode.metadata() {
                Ok(m) => m,
                Err(e) => return e.to_errno(),
            };

            // 转换为绝对偏移
            let file_offset = file.offset();
            let file_size = metadata.size;
            let (start, len) = match flock.to_absolute_range(file_offset, file_size) {
                Ok(range) => range,
                Err(_) => return -(EINVAL as isize),
            };

            // 获取当前进程 PID
            let pid = task.lock().pid as i32;

            // 设置锁
            // TODO: 获取真实设备号（与上面 F_GETLK 的 TODO 相同）
            let dev = 0;
            let ino = metadata.inode_no as u64;
            match file_lock_manager().set_lock(dev, ino, start, len, lock_type, pid, blocking) {
                Ok(()) => 0,
                Err(e) => e.to_errno(),
            }
        }

        //  异步 I/O 和信号
        FcntlCmd::GetOwn => {
            // F_GETOWN: 获取异步 I/O 所有者
            let file = match task.lock().fd_table.get(fd) {
                Ok(f) => f,
                Err(e) => return e.to_errno(),
            };

            match file.get_owner() {
                Ok(pid) => pid as isize,
                Err(e) => e.to_errno(),
            }
        }

        FcntlCmd::SetOwn => {
            // F_SETOWN: 设置异步 I/O 所有者
            let pid = arg as i32;
            let file = match task.lock().fd_table.get(fd) {
                Ok(f) => f,
                Err(e) => return e.to_errno(),
            };

            match file.set_owner(pid) {
                Ok(()) => 0,
                Err(e) => e.to_errno(),
            }
        }

        FcntlCmd::SetSig | FcntlCmd::GetSig => {
            // F_SETSIG / F_GETSIG: 设置/获取异步 I/O 信号
            // 默认是 SIGIO，暂不支持自定义信号
            FsError::NotSupported.to_errno()
        }

        //  管道大小
        FcntlCmd::GetPipeSz => {
            // F_GETPIPE_SZ: 获取管道大小
            let file = match task.lock().fd_table.get(fd) {
                Ok(f) => f,
                Err(e) => return e.to_errno(),
            };

            match file.get_pipe_size() {
                Ok(size) => size as isize,
                Err(e) => e.to_errno(),
            }
        }

        FcntlCmd::SetPipeSz => {
            // F_SETPIPE_SZ: 设置管道大小
            let new_size = arg;
            let file = match task.lock().fd_table.get(fd) {
                Ok(f) => f,
                Err(e) => return e.to_errno(),
            };

            match file.set_pipe_size(new_size) {
                Ok(()) => new_size as isize,
                Err(e) => e.to_errno(),
            }
        }
    }
}

/// F_DUPFD / F_DUPFD_CLOEXEC 的辅助函数
fn fcntl_dupfd(
    task: &Arc<crate::sync::SpinLock<crate::kernel::task::TaskStruct>>,
    old_fd: usize,
    min_fd: usize,
    cloexec: bool,
) -> isize {
    let flags = if cloexec {
        FdFlags::CLOEXEC
    } else {
        FdFlags::empty()
    };

    // 使用 FDTable 的标准方法进行复制
    let new_fd = match task.lock().fd_table.dup_from(old_fd, min_fd, flags) {
        Ok(fd) => {
            crate::earlyprintln!(
                "fcntl_dupfd: old_fd={} -> new_fd={}, min_fd={}, cloexec={}",
                old_fd,
                fd,
                min_fd,
                cloexec
            );
            fd
        }
        Err(e) => {
            crate::earlyprintln!("fcntl_dupfd: failed: {:?}", e);
            return e.to_errno();
        }
    };

    new_fd as isize
}
