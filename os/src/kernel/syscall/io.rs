//! IO 相关的系统调用实现

use crate::arch::Arch;
use crate::kernel::current_task;
use crate::uapi::errno::EFAULT;
use crate::uapi::errno::EINVAL;
use crate::uapi::iovec::IoVec;
use crate::util::user_buffer::{read_from_user, validate_user_ptr, validate_user_ptr_mut, write_to_user};
use crate::vfs::File;

/// 向文件描述符写入数据
pub fn write(fd: usize, buf: *const u8, count: usize) -> isize {
    loop {
        let task = current_task();
        let file = match task.lock().fd_table.get(fd) {
            Ok(f) => f,
            Err(e) => return e.to_errno(),
        };

        let mut kernel_buf = alloc::vec![0u8; count];
        unsafe {
            crate::arch::ArchImpl::copy_from_user(
                buf as usize,
                kernel_buf.as_mut_ptr(),
                count,
            )
            .ok();
        }

        let result = match file.write(&kernel_buf) {
            Ok(n) => n as isize,
            Err(e) => e.to_errno(),
        };

        if result == -11 {
            use crate::net::socket::SocketFile;
            if let Some(socket_file) = file.as_any().downcast_ref::<SocketFile>()
                && !socket_file
                    .flags()
                    .contains(crate::uapi::fcntl::OpenFlags::O_NONBLOCK)
            {
                drop(file);
                drop(task);
                crate::net::socket::poll_network_and_dispatch();
                crate::kernel::yield_task();
                continue;
            }
        }

        return result;
    }
}

/// 从文件描述符读取数据
pub fn read(fd: usize, buf: *mut u8, count: usize) -> isize {
    loop {
        let task = current_task();
        let file = match task.lock().fd_table.get(fd) {
            Ok(f) => f,
            Err(e) => return e.to_errno(),
        };

        let mut kernel_buf = alloc::vec![0u8; count];
        let result = match file.read(&mut kernel_buf) {
            Ok(n) => {
                unsafe {
                    crate::arch::ArchImpl::copy_to_user(
                        kernel_buf.as_ptr(),
                        buf as usize,
                        n,
                    )
                    .ok();
                }
                n as isize
            }
            Err(e) => e.to_errno(),
        };

        if result == -11 {
            use crate::net::socket::SocketFile;
            if let Some(socket_file) = file.as_any().downcast_ref::<SocketFile>()
                && !socket_file
                    .flags()
                    .contains(crate::uapi::fcntl::OpenFlags::O_NONBLOCK)
            {
                drop(file);
                drop(task);
                crate::net::socket::poll_network_and_dispatch();
                crate::kernel::yield_task();
                continue;
            }
        }

        return result;
    }
}

/// 向量化读取：从文件描述符读取数据到多个缓冲区
pub fn readv(fd: usize, iov: *const IoVec, iovcnt: usize) -> isize {
    if iov.is_null() || iovcnt == 0 || iovcnt > 1024 {
        return -(EINVAL as isize);
    }

    if !validate_user_ptr(iov) {
        return -(EFAULT as isize);
    }

    // 读取 iovec 数组
    let mut iovec_array = alloc::vec::Vec::<IoVec>::with_capacity(iovcnt);
    unsafe {
        iovec_array.set_len(iovcnt);
        crate::arch::ArchImpl::copy_from_user(
            iov as usize,
            iovec_array.as_mut_ptr() as *mut u8,
            iovcnt * core::mem::size_of::<IoVec>(),
        ).ok();
    }

    let task = current_task();
    let file = match task.lock().fd_table.get(fd) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    let mut total_read = 0usize;
    for vec in &iovec_array {
        if vec.iov_base.is_null() || vec.iov_len == 0 {
            continue;
        }

        if !validate_user_ptr_mut(vec.iov_base) {
            return if total_read > 0 {
                total_read as isize
            } else {
                -(EFAULT as isize)
            };
        }

        let mut kernel_buf = alloc::vec![0u8; vec.iov_len];
        match file.read(&mut kernel_buf) {
            Ok(n) => {
                unsafe {
                    crate::arch::ArchImpl::copy_to_user(
                        kernel_buf.as_ptr(),
                        vec.iov_base as usize,
                        n,
                    ).ok();
                }
                total_read += n;
                if n < vec.iov_len {
                    break;
                }
            }
            Err(e) => {
                return if total_read > 0 {
                    total_read as isize
                } else {
                    e.to_errno()
                };
            }
        }
    }

    total_read as isize
}

/// 向量化写入：将多个缓冲区的数据写入文件描述符
pub fn writev(fd: usize, iov: *const IoVec, iovcnt: usize) -> isize {
    if iov.is_null() || iovcnt == 0 || iovcnt > 1024 {
        return -(EINVAL as isize);
    }

    if !validate_user_ptr(iov) {
        return -(EFAULT as isize);
    }

    let mut iovec_array = alloc::vec::Vec::<IoVec>::with_capacity(iovcnt);
    unsafe {
        iovec_array.set_len(iovcnt);
        crate::arch::ArchImpl::copy_from_user(
            iov as usize,
            iovec_array.as_mut_ptr() as *mut u8,
            iovcnt * core::mem::size_of::<IoVec>(),
        ).ok();
    }

    let task = current_task();
    let file = match task.lock().fd_table.get(fd) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    let mut total_written = 0usize;
    for vec in &iovec_array {
        if vec.iov_base.is_null() || vec.iov_len == 0 {
            continue;
        }

        if !validate_user_ptr(vec.iov_base) {
            return if total_written > 0 {
                total_written as isize
            } else {
                -(EFAULT as isize)
            };
        }

        let mut kernel_buf = alloc::vec![0u8; vec.iov_len];
        unsafe {
            crate::arch::ArchImpl::copy_from_user(
                vec.iov_base as usize,
                kernel_buf.as_mut_ptr(),
                vec.iov_len,
            ).ok();
        }
        match file.write(&kernel_buf) {
            Ok(n) => {
                total_written += n;
                if n < vec.iov_len {
                    break;
                }
            }
            Err(e) => {
                return if total_written > 0 {
                    total_written as isize
                } else {
                    e.to_errno()
                };
            }
        }
    }

    total_written as isize
}

/// 位置读取：从指定位置读取数据，不改变文件偏移量
/// # 参数
/// - `fd`: 文件描述符
/// - `buf`: 存储读取数据的缓冲区
/// - `count`: 要读取的字节数
/// - `offset`: 文件偏移量
pub fn pread64(fd: usize, buf: *mut u8, count: usize, offset: i64) -> isize {
    if offset < 0 {
        return -(EINVAL as isize);
    }

    let task = current_task();
    let file = match task.lock().fd_table.get(fd) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    let mut kernel_buf = alloc::vec![0u8; count];
    match file.read_at(offset as usize, &mut kernel_buf) {
        Ok(n) => {
            unsafe {
                crate::arch::ArchImpl::copy_to_user(
                    kernel_buf.as_ptr(),
                    buf as usize,
                    n,
                ).ok();
            }
            n as isize
        }
        Err(e) => e.to_errno(),
    }
}

/// 位置写入：向指定位置写入数据，不改变文件偏移量
/// # 参数
/// - `fd`: 文件描述符
/// - `buf`: 要写入的数据缓冲区
/// - `count`: 要写入的字节数
/// - `offset`: 文件偏移量
pub fn pwrite64(fd: usize, buf: *const u8, count: usize, offset: i64) -> isize {
    if offset < 0 {
        return -(EINVAL as isize);
    }

    let task = current_task();
    let file = match task.lock().fd_table.get(fd) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    let mut kernel_buf = alloc::vec![0u8; count];
    unsafe {
        crate::arch::ArchImpl::copy_from_user(
            buf as usize,
            kernel_buf.as_mut_ptr(),
            count,
        ).ok();
    }
    match file.write_at(offset as usize, &kernel_buf) {
        Ok(n) => n as isize,
        Err(e) => e.to_errno(),
    }
}

/// 向量化位置读取：从指定位置读取数据到多个缓冲区，不改变文件偏移量
/// # 参数
/// - `fd`: 文件描述符
/// - `iov`: iovec 数组指针
/// - `iovcnt`: iovec 数组元素个数
/// - `offset`: 文件偏移量
pub fn preadv(fd: usize, iov: *const IoVec, iovcnt: usize, offset: i64) -> isize {
    if iov.is_null() || iovcnt == 0 || iovcnt > 1024 || offset < 0 {
        return -(EINVAL as isize);
    }

    // 验证 iovec 数组指针
    if !validate_user_ptr(iov) {
        return -(EFAULT as isize);
    }

    let task = current_task();
    let file = match task.lock().fd_table.get(fd) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    let mut iovec_array = alloc::vec::Vec::<IoVec>::with_capacity(iovcnt);
    unsafe {
        iovec_array.set_len(iovcnt);
        crate::arch::ArchImpl::copy_from_user(
            iov as usize,
            iovec_array.as_mut_ptr() as *mut u8,
            iovcnt * core::mem::size_of::<IoVec>(),
        ).ok();
    }

    let mut total_read = 0usize;
    let mut current_offset = offset as usize;
    for vec in &iovec_array {
        if vec.iov_base.is_null() || vec.iov_len == 0 {
            continue;
        }

        if !validate_user_ptr_mut(vec.iov_base) {
            return if total_read > 0 {
                total_read as isize
            } else {
                -(EFAULT as isize)
            };
        }

        let mut kernel_buf = alloc::vec![0u8; vec.iov_len];
        match file.read_at(current_offset, &mut kernel_buf) {
            Ok(n) => {
                unsafe {
                    crate::arch::ArchImpl::copy_to_user(
                        kernel_buf.as_ptr(),
                        vec.iov_base as usize,
                        n,
                    ).ok();
                }
                total_read += n;
                current_offset += n;
                if n < vec.iov_len {
                    break;
                }
            }
            Err(e) => {
                return if total_read > 0 {
                    total_read as isize
                } else {
                    e.to_errno()
                };
            }
        }
    }

    total_read as isize
}

/// 向量化位置写入：将多个缓冲区的数据写入指定位置，不改变文件偏移量
/// # 参数
/// - `fd`: 文件描述符
/// - `iov`: iovec 数组指针
/// - `iovcnt`: iovec 数组元素个数
/// - `offset`: 文件偏移量
pub fn pwritev(fd: usize, iov: *const IoVec, iovcnt: usize, offset: i64) -> isize {
    if iov.is_null() || iovcnt == 0 || iovcnt > 1024 || offset < 0 {
        return -(EINVAL as isize);
    }

    // 验证 iovec 数组指针
    if !validate_user_ptr(iov) {
        return -(EFAULT as isize);
    }

    let task = current_task();
    let file = match task.lock().fd_table.get(fd) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    let mut iovec_array = alloc::vec::Vec::<IoVec>::with_capacity(iovcnt);
    unsafe {
        iovec_array.set_len(iovcnt);
        crate::arch::ArchImpl::copy_from_user(
            iov as usize,
            iovec_array.as_mut_ptr() as *mut u8,
            iovcnt * core::mem::size_of::<IoVec>(),
        ).ok();
    }

    let mut total_written = 0usize;
    let mut current_offset = offset as usize;
    for vec in &iovec_array {
        if vec.iov_base.is_null() || vec.iov_len == 0 {
            continue;
        }

        if !validate_user_ptr(vec.iov_base) {
            return if total_written > 0 {
                total_written as isize
            } else {
                -(EFAULT as isize)
            };
        }

        let mut kernel_buf = alloc::vec![0u8; vec.iov_len];
        unsafe {
            crate::arch::ArchImpl::copy_from_user(
                vec.iov_base as usize,
                kernel_buf.as_mut_ptr(),
                vec.iov_len,
            ).ok();
        }
        match file.write_at(current_offset, &kernel_buf) {
            Ok(n) => {
                total_written += n;
                current_offset += n;
                if n < vec.iov_len {
                    break;
                }
            }
            Err(e) => {
                return if total_written > 0 {
                    total_written as isize
                } else {
                    e.to_errno()
                };
            }
        }
    }

    total_written as isize
}

/// 零拷贝文件传输：从一个文件描述符传输数据到另一个
/// # 参数
/// - `out_fd`: 输出文件描述符
/// - `in_fd`: 输入文件描述符
/// - `offset`: 输入文件偏移量指针（如果非空，从该位置读取并更新）
/// - `count`: 要传输的字节数
pub fn sendfile(out_fd: usize, in_fd: usize, offset: *mut i64, count: usize) -> isize {
    let task = current_task();

    let in_file = match task.lock().fd_table.get(in_fd) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    let out_file = match task.lock().fd_table.get(out_fd) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    // 如果 offset 非空，使用 pread；否则使用 read
    let use_offset = !offset.is_null();
    let mut current_offset = if use_offset {
        let off = unsafe { read_from_user(offset) };
        if off < 0 {
            return -(EINVAL as isize);
        }
        off as usize
    } else {
        0
    };

    // 使用 8KB 缓冲区进行传输
    const BUFFER_SIZE: usize = 8192;
    let mut buffer = [0u8; BUFFER_SIZE];
    let mut total_sent = 0usize;
    let mut remaining = count;

    while remaining > 0 {
        let to_read = core::cmp::min(remaining, BUFFER_SIZE);

        // 读取数据
        let read_result = if use_offset {
            in_file.read_at(current_offset, &mut buffer[..to_read])
        } else {
            in_file.read(&mut buffer[..to_read])
        };

        let n_read = match read_result {
            Ok(0) => break, // EOF
            Ok(n) => n,
            Err(e) => {
                return if total_sent > 0 {
                    total_sent as isize
                } else {
                    e.to_errno()
                };
            }
        };

        // 写入数据
        match out_file.write(&buffer[..n_read]) {
            Ok(n_written) => {
                total_sent += n_written;
                if use_offset {
                    current_offset += n_written;
                }
                remaining -= n_written;
                if n_written < n_read {
                    break; // 输出端写不完
                }
            }
            Err(e) => {
                return if total_sent > 0 {
                    total_sent as isize
                } else {
                    e.to_errno()
                };
            }
        }
    }

    // 更新 offset 指针
    if use_offset {
        unsafe { write_to_user(offset, current_offset as i64) };
    }

    total_sent as isize
}

/// pollfd 结构体
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PollFd {
    pub fd: i32,
    pub events: i16,
    pub revents: i16,
}

/// poll 事件标志
pub const POLLIN: i16 = 0x0001;
pub const POLLOUT: i16 = 0x0004;
pub const POLLERR: i16 = 0x0008;
pub const POLLHUP: i16 = 0x0010;
pub const POLLNVAL: i16 = 0x0020;

use crate::kernel::scheduler::WaitQueue;
use crate::sync::SpinLock;
use lazy_static::lazy_static;

lazy_static! {
    static ref POLL_WAIT_QUEUE: SpinLock<WaitQueue> = SpinLock::new(WaitQueue::new());
}

/// Wake up all tasks waiting in poll
pub fn wake_poll_waiters() {
    POLL_WAIT_QUEUE.lock().wake_up_all();
}

fn poll_with_timeout(
    fds: usize,
    nfds: usize,
    timeout: Option<crate::uapi::time::TimeSpec>,
) -> isize {
    use crate::uapi::errno::{EINTR, EINVAL};

    if nfds > 0 && fds == 0 {
        return -(EINVAL as isize);
    }

    let task = current_task();

    let timeout_trigger = match timeout {
        None => None,
        Some(ts) => {
            if ts.tv_sec < 0 || ts.tv_nsec < 0 || ts.tv_nsec >= 1_000_000_000 {
                return -(EINVAL as isize);
            }
            let duration_ns = (ts.tv_sec as u64 * 1_000_000_000) + ts.tv_nsec as u64;
            let duration_ticks = (duration_ns * crate::arch::clock_freq() as u64 / 1_000_000_000) as usize;
            Some(crate::arch::get_time() + duration_ticks)
        }
    };

    loop {
        // 关键：在阻塞等待前主动推进网络栈，并分发 UDP 到每个 fd 的队列，避免“永远等不到”
        crate::net::socket::poll_network_and_dispatch();

        let mut ready_count = 0;

        {
            let size = nfds * core::mem::size_of::<PollFd>();
            let mut pollfds_buf = alloc::vec::Vec::<PollFd>::with_capacity(nfds);
            unsafe {
                pollfds_buf.set_len(nfds);
                crate::arch::ArchImpl::copy_from_user(
                    fds,
                    pollfds_buf.as_mut_ptr() as *mut u8,
                    size,
                ).ok();
            }

            for pollfd in pollfds_buf.iter_mut() {
                pollfd.revents = 0;

                if pollfd.fd < 0 {
                    continue;
                }

                let file = match task.lock().fd_table.get(pollfd.fd as usize) {
                    Ok(f) => f,
                    Err(_) => {
                        pollfd.revents = POLLNVAL;
                        ready_count += 1;
                        continue;
                    }
                };

                if (pollfd.events & POLLIN) != 0 && file.readable() {
                    pollfd.revents |= POLLIN;
                }

                if (pollfd.events & POLLOUT) != 0 && file.writable() {
                    pollfd.revents |= POLLOUT;
                }

                if pollfd.revents != 0 {
                    ready_count += 1;
                }
            }

            // Write revents back to user space
            unsafe {
                crate::arch::ArchImpl::copy_to_user(
                    pollfds_buf.as_ptr() as *const u8,
                    fds,
                    size,
                ).ok();
            }
        }

        if ready_count > 0 {
            return ready_count;
        }

        if crate::ipc::signal_interrupts_syscall(&task) {
            return -(EINTR as isize);
        }

        if let Some(trigger) = timeout_trigger {
            use crate::kernel::timer::TIMER_QUEUE;
            let mut timer_q = TIMER_QUEUE.lock();
            timer_q.push(trigger, task.clone());
            drop(timer_q);
        }

        POLL_WAIT_QUEUE.lock().sleep(task.clone());
        crate::kernel::schedule();

        if timeout_trigger.is_some() {
            use crate::kernel::timer::TIMER_QUEUE;
            TIMER_QUEUE.lock().remove_task(&task);
        }

        if crate::ipc::signal_interrupts_syscall(&task) {
            return -(EINTR as isize);
        }

        // 被唤醒后再推进一次网络栈，并把“刚到的数据包”分发成 socket 可读事件
        crate::net::socket::poll_network_and_dispatch();

        // Check if woken by timeout
        if let Some(trigger) = timeout_trigger
            && crate::arch::get_time() >= trigger
        {
            return 0;
        }
    }
}

/// ppoll - poll 的变体，支持信号掩码
pub fn ppoll(fds: usize, nfds: usize, timeout: usize, _sigmask: usize) -> isize {
    use crate::uapi::errno::EINVAL;

    if nfds > 0 && fds == 0 {
        return -(EINVAL as isize);
    }

    let timeout_spec = if timeout == 0 {
        None
    } else {
        let ts: crate::uapi::time::TimeSpec = read_from_user(timeout as *const crate::uapi::time::TimeSpec);
        if ts.tv_nsec < 0 || ts.tv_nsec >= 1_000_000_000 {
            return -(EINVAL as isize);
        }
        Some(ts)
    };

    poll_with_timeout(fds, nfds, timeout_spec)
}

/// pselect6 - synchronous I/O multiplexing with signal mask
/// Note: sigmask handling requires signal subsystem refactoring, currently ignored
pub fn pselect6(
    nfds: usize,
    readfds: usize,
    writefds: usize,
    exceptfds: usize,
    timeout: usize,
    _sigmask: usize,
) -> isize {
    use crate::uapi::errno::EINVAL;
    use crate::uapi::time::TimeSpec;

    // pselect6 uses `timespec*` (tv_nsec), NOT `timeval*` (tv_usec).
    let timeout_trigger = if timeout == 0 {
        None // Infinite timeout
    } else {
        let ts: TimeSpec = read_from_user(timeout as *const TimeSpec);
        if ts.tv_sec < 0 || ts.tv_nsec < 0 || ts.tv_nsec >= 1_000_000_000 {
            return -(EINVAL as isize);
        }
        if ts.is_zero() {
            Some(0) // Poll mode (no wait)
        } else {
            let duration_ticks = ts.into_freq(crate::arch::clock_freq());
            Some(crate::arch::get_time() + duration_ticks)
        }
    };

    // TODO: Implement signal mask handling when signal subsystem is refactored.
    select_common(nfds, readfds, writefds, exceptfds, timeout_trigger)
}

/// select - synchronous I/O multiplexing
pub fn select(
    nfds: usize,
    readfds: usize,
    writefds: usize,
    exceptfds: usize,
    timeout: usize,
) -> isize {
    use crate::uapi::errno::EINVAL;
    use crate::uapi::time::timeval;

    // Parse timeout (select uses `timeval*`)
    let timeout_trigger = if timeout == 0 {
        None // Infinite timeout
    } else {
        let tv: timeval = read_from_user(timeout as *const timeval);
        if tv.tv_sec < 0 || tv.tv_usec < 0 || tv.tv_usec >= 1_000_000 {
            return -(EINVAL as isize);
        }
        if tv.is_zero() {
            Some(0) // Poll mode (no wait)
        } else {
            let duration_ticks = tv.into_freq(crate::arch::clock_freq());
            Some(crate::arch::get_time() + duration_ticks)
        }
    };

    select_common(nfds, readfds, writefds, exceptfds, timeout_trigger)
}

fn select_common(
    nfds: usize,
    readfds: usize,
    writefds: usize,
    exceptfds: usize,
    timeout_trigger: Option<usize>,
) -> isize {
    use crate::kernel::current_task;
    use crate::uapi::errno::{EBADF, EINTR, EINVAL};
    use crate::uapi::select::FdSet;

    if nfds > crate::uapi::select::FD_SETSIZE {
        return -(EINVAL as isize);
    }

    let task = current_task();

    // Copy input fd_sets once before loop
    let (input_read, input_write, input_except) = (
        if readfds != 0 {
            Some(read_from_user(readfds as *const FdSet))
        } else {
            None
        },
        if writefds != 0 {
            Some(read_from_user(writefds as *const FdSet))
        } else {
            None
        },
        if exceptfds != 0 {
            Some(read_from_user(exceptfds as *const FdSet))
        } else {
            None
        },
    );

    // Helper to check fds
    let check_fds = || -> (isize, Option<FdSet>, Option<FdSet>, Option<FdSet>) {
        let mut ready_count = 0;
        let mut read_set = input_read.as_ref().map(|_| FdSet::new());
        let mut write_set = input_write.as_ref().map(|_| FdSet::new());
        let except_set = input_except.as_ref().map(|_| FdSet::new());

        let task_lock = task.lock();
        for fd in 0..nfds {
            let check_read = input_read.as_ref().is_some_and(|s| s.is_set(fd));
            let check_write = input_write.as_ref().is_some_and(|s| s.is_set(fd));
            let check_except = input_except.as_ref().is_some_and(|s| s.is_set(fd));

            if !check_read && !check_write && !check_except {
                continue;
            }

            let file = match task_lock.fd_table.get(fd) {
                Ok(f) => f,
                Err(_) => {
                    crate::pr_warn!(
                        "select: EBADF tid={}, fd={}, check_read={}, check_write={}, check_except={}",
                        task_lock.tid,
                        fd,
                        check_read,
                        check_write,
                        check_except
                    );
                    return (-(EBADF as isize), None, None, None);
                }
            };

            let mut fd_ready = false;
            if check_read
                && file.readable()
                && let Some(ref mut set) = read_set
            {
                set.set(fd);
                fd_ready = true;
            }
            if check_write
                && file.writable()
                && let Some(ref mut set) = write_set
            {
                set.set(fd);
                fd_ready = true;
            }
            // exceptfds: OOB data, errors (not implemented yet)
            if fd_ready {
                ready_count += 1;
            }
        }
        (ready_count, read_set, write_set, except_set)
    };

    loop {
        // 关键：在阻塞等待前主动推进网络栈（同 ppoll），并分发 UDP
        crate::net::socket::poll_network_and_dispatch();

        let (ready_count, read_set, write_set, except_set) = check_fds();
        if ready_count < 0 {
            return ready_count;
        } // EBADF

        if ready_count > 0 {
            if let Some(ref set) = read_set {
                write_to_user(readfds as *mut FdSet, *set);
            }
            if let Some(ref set) = write_set {
                write_to_user(writefds as *mut FdSet, *set);
            }
            if let Some(ref set) = except_set {
                write_to_user(exceptfds as *mut FdSet, *set);
            }
            return ready_count;
        }

        // If interrupted by a deliverable signal, return EINTR so userland can run the handler.
        // Signals are only checked on return-to-user; without this, we can sleep forever in-kernel.
        if crate::ipc::signal_interrupts_syscall(&task) {
            return -(EINTR as isize);
        }

        if let Some(0) = timeout_trigger {
            return 0;
        }

        if let Some(trigger) = timeout_trigger {
            use crate::kernel::timer::TIMER_QUEUE;
            TIMER_QUEUE.lock().push(trigger, task.clone());
        }

        // Atomic check-and-sleep to prevent lost wakeup
        let slept = {
            let mut wq = POLL_WAIT_QUEUE.lock();
            wq.sleep_if(task.clone(), || {
                let (ready, _, _, _) = check_fds();
                ready > 0
            })
        };

        if slept {
            crate::kernel::schedule();

            if timeout_trigger.is_some() {
                use crate::kernel::timer::TIMER_QUEUE;
                TIMER_QUEUE.lock().remove_task(&task);
            }

            if crate::ipc::signal_interrupts_syscall(&task) {
                return -(EINTR as isize);
            }

            // 被唤醒后再推进一次网络栈，并分发 UDP
            crate::net::socket::poll_network_and_dispatch();

            if let Some(trigger) = timeout_trigger {
                if crate::arch::get_time() >= trigger {
                    return 0;
                }
            }
        }
    }
}

/// poll - I/O 多路复用系统调用
/// # 参数
/// - `fds`: pollfd 数组指针
/// - `nfds`: 数组长度
/// - `timeout`: 超时时间（毫秒），-1 表示无限等待，0 表示立即返回
pub fn poll(fds: usize, nfds: usize, timeout: i32) -> isize {
    let timeout_spec = if timeout < 0 {
        None
    } else {
        Some(crate::uapi::time::TimeSpec {
            tv_sec: (timeout / 1000) as i64,
            tv_nsec: ((timeout % 1000) * 1_000_000) as i64,
        })
    };

    poll_with_timeout(fds, nfds, timeout_spec)
}
