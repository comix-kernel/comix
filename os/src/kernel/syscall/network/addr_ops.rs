use super::*;

// 接受连接（非阻塞）
pub fn accept4(sockfd: i32, addr: *mut u8, addrlen: *mut u32, flags: i32) -> isize {
    let supported_flags = SOCK_NONBLOCK | SOCK_CLOEXEC;
    if flags & !supported_flags != 0 {
        return -22; // EINVAL
    }

    let fd = accept(sockfd, addr, addrlen);
    if fd < 0 {
        return fd;
    }

    let task = current_task();
    let fd_usize = fd as usize;

    if flags & SOCK_CLOEXEC != 0 {
        let task_lock = task.lock();
        if let Ok(old_flags) = task_lock.fd_table.get_fd_flags(fd_usize) {
            let _ = task_lock
                .fd_table
                .set_fd_flags(fd_usize, old_flags | FdFlags::CLOEXEC);
        }
    }

    if flags & SOCK_NONBLOCK != 0 {
        let task_lock = task.lock();
        if let Ok(file) = task_lock.fd_table.get(fd_usize) {
            let mut new_flags = file.flags();
            new_flags |= OpenFlags::O_NONBLOCK;
            let _ = file.set_status_flags(new_flags);
        }
    }

    fd
}

// 发送数据到指定地址
pub fn sendto(
    sockfd: i32,
    buf: *const u8,
    len: usize,
    _flags: i32,
    dest_addr: *const u8,
    addrlen: u32,
) -> isize {
    pr_debug!("sendto: sockfd={}, len={}", sockfd, len);
    // If dest_addr is null, behave like send()
    if dest_addr.is_null() {
        return send(sockfd, buf, len, 0);
    }

    let endpoint = match parse_sockaddr_in(dest_addr, addrlen) {
        Ok(e) => e,
        Err(_) => return -22, // EINVAL
    };

    let task = current_task();
    let tid = task.lock().tid as usize;

    let handle = match get_socket_handle(tid, sockfd as usize) {
        Some(h) => h,
        None => return -88, // ENOTSOCK
    };

    use crate::net::socket::socket_sendto;
    let result = {
        let mut kernel_buf = alloc::vec![0u8; len];
        unsafe {
            crate::arch::ArchImpl::copy_from_user(
                crate::arch::address::UA::from_usize(buf as usize),
                kernel_buf.as_mut_ptr(),
                len,
            )
            .ok();
        }
        socket_sendto(handle, &kernel_buf, endpoint)
    };
    match result {
        Ok(n) => {
            pr_debug!(
                "sendto: sockfd={}, len={}, endpoint={} -> sent={}",
                sockfd,
                len,
                endpoint,
                n
            );
            n as isize
        }
        Err(e) => {
            pr_debug!(
                "sendto: sockfd={}, len={}, endpoint={} -> error={:?}",
                sockfd,
                len,
                endpoint,
                e
            );
            e.to_errno()
        }
    }
}

// Linux 标准: ssize_t recvfrom(int sockfd, void *buf, size_t len, int flags, struct sockaddr *src_addr, socklen_t *addrlen);
pub fn recvfrom(
    sockfd: i32,
    buf: *mut u8,
    len: usize,
    _flags: i32,
    src_addr: *mut u8,
    addrlen: *mut u32,
) -> isize {
    pr_debug!("recvfrom: sockfd={}, len={}", sockfd, len);
    loop {
        let task = current_task();
        let file = match task.lock().fd_table.get(sockfd as usize) {
            Ok(f) => f,
            Err(_) => return -9, // EBADF
        };

        let result = {
            let mut kernel_buf = alloc::vec![0u8; len];
            match file.recvfrom(&mut kernel_buf) {
                Ok((n, addr)) => {
                    unsafe {
                        crate::arch::ArchImpl::copy_to_user(
                            kernel_buf.as_ptr(),
                            crate::arch::address::UA::from_usize(buf as usize),
                            n,
                        )
                        .ok();
                    }
                    Ok((n, addr))
                }
                Err(e) => Err(e),
            }
        };

        match result {
            Ok((n, Some(addr_buf))) => {
                if !src_addr.is_null() && !addrlen.is_null() {
                    let user_addrlen = read_from_user(addrlen as *const u32) as usize;
                    let copy_len = user_addrlen.min(addr_buf.len());
                    unsafe {
                        crate::arch::ArchImpl::copy_to_user(
                            addr_buf.as_ptr(),
                            crate::arch::address::UA::from_usize(src_addr as usize),
                            copy_len,
                        )
                        .ok();
                    }
                    write_to_user(addrlen, copy_len as u32);
                }
                pr_debug!(
                    "recvfrom: sockfd={}, len={} -> received={} (with addr)",
                    sockfd,
                    len,
                    n
                );
                return n as isize;
            }
            Ok((n, None)) => {
                pr_debug!(
                    "recvfrom: sockfd={}, len={} -> received={} (no addr)",
                    sockfd,
                    len,
                    n
                );
                return n as isize;
            }
            Err(e) => {
                pr_debug!("recvfrom: sockfd={}, len={} -> error={:?}", sockfd, len, e);
                if e == crate::vfs::FsError::WouldBlock {
                    use crate::net::socket::SocketFile;
                    if let Some(socket_file) = file.as_any().downcast_ref::<SocketFile>()
                        && !socket_file
                            .flags()
                            .contains(crate::uapi::fcntl::OpenFlags::O_NONBLOCK)
                    {
                        drop(file);
                        crate::net::socket::poll_network_and_dispatch();
                        crate::kernel::yield_task();
                        if crate::ipc::signal_interrupts_syscall(&task) {
                            return -(crate::uapi::errno::EINTR as isize);
                        }
                        continue;
                    }
                }
                return e.to_errno();
            }
        }
    }
}

// 关闭套接字
pub fn shutdown(sockfd: i32, how: i32) -> isize {
    const SHUT_RD: i32 = 0;
    const SHUT_WR: i32 = 1;
    const SHUT_RDWR: i32 = 2;

    if !(0..=2).contains(&how) {
        return -22; // EINVAL
    }

    let task = current_task();
    let task_lock = task.lock();
    let tid = task_lock.tid as usize;

    let handle = match get_socket_handle(tid, sockfd as usize) {
        Some(h) => h,
        None => return -88, // ENOTSOCK
    };

    let file = match task_lock.fd_table.get(sockfd as usize) {
        Ok(f) => f,
        Err(_) => return -9, // EBADF
    };
    drop(task_lock);

    use crate::net::socket::{socket_shutdown_read, socket_shutdown_write};

    let should_close_tcp = match how {
        SHUT_RD => {
            socket_shutdown_read(&file);
            false
        }
        SHUT_WR | SHUT_RDWR => {
            if how == SHUT_RDWR {
                socket_shutdown_read(&file);
            }
            socket_shutdown_write(&file);
            true
        }
        _ => unreachable!(), // 这里是不可到达的到达即意味着有问题
    };

    if should_close_tcp && let SocketHandle::Tcp(h) = handle {
        network_stack().tcp_close(h);
    }

    0
}

// 获取套接字地址
pub fn getsockname(sockfd: i32, addr: *mut u8, addrlen: *mut u32) -> isize {
    let task = current_task();
    let task_lock = task.lock();
    let tid = task_lock.tid as usize;

    let handle = match get_socket_handle(tid, sockfd as usize) {
        Some(h) => h,
        None => return -88, // ENOTSOCK
    };
    let file = match task_lock.fd_table.get(sockfd as usize) {
        Ok(f) => f,
        Err(_) => return -9, // EBADF
    };
    drop(task_lock);

    let local_endpoint = network_stack().socket_local_endpoint(handle);

    // Linux behavior: getsockname() on an unbound socket typically returns success and
    // fills a sockaddr with AF_INET and port 0.
    let ep = match local_endpoint {
        Some(ep) => ep,
        None => {
            use crate::net::socket::get_socket_local_endpoint;
            get_socket_local_endpoint(&file)
                .unwrap_or_else(|| IpEndpoint::new(IpAddress::Ipv4(Ipv4Address::UNSPECIFIED), 0))
        }
    };

    if write_sockaddr_in(addr, addrlen, ep).is_err() {
        return -22; // EINVAL
    }
    0
}

// 获取对端套接字地址
pub fn getpeername(sockfd: i32, addr: *mut u8, addrlen: *mut u32) -> isize {
    let task = current_task();
    let tid = task.lock().tid as usize;

    let handle = match get_socket_handle(tid, sockfd as usize) {
        Some(h) => h,
        None => return -88, // ENOTSOCK
    };

    let remote_endpoint = match handle {
        SocketHandle::Tcp(_) => network_stack().socket_remote_endpoint(handle),
        SocketHandle::Udp(_) => {
            // UDP doesn't have a peer, use stored endpoint
            let file = match task.lock().fd_table.get(sockfd as usize) {
                Ok(f) => f,
                Err(_) => return -9, // EBADF
            };
            use crate::net::socket::get_socket_remote_endpoint;
            get_socket_remote_endpoint(&file)
        }
    };

    if let Some(ep) = remote_endpoint {
        if write_sockaddr_in(addr, addrlen, ep).is_err() {
            return -22; // EINVAL
        }
        0
    } else {
        -107 // ENOTCONN
    }
}
