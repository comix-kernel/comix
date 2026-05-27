use super::*;

/// 连接到远程地址
pub fn connect(sockfd: i32, addr: *const u8, addrlen: u32) -> isize {
    let endpoint = match parse_sockaddr_in(addr, addrlen) {
        Ok(e) => {
            pr_debug!("connect: sockfd={}, endpoint={}", sockfd, e);
            e
        }
        Err(e) => return e.to_errno(),
    };

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

    use crate::net::socket::set_socket_remote_endpoint;
    if let Err(e) = set_socket_remote_endpoint(&file, endpoint) {
        return e.to_errno();
    }

    let is_nonblock = file
        .flags()
        .contains(crate::uapi::fcntl::OpenFlags::O_NONBLOCK);

    match handle {
        SocketHandle::Tcp(h) => {
            let (_state, is_open) = network_stack()
                .tcp_debug_state(h)
                .unwrap_or((TcpConnectionState::Closed, false));
            pr_debug!("connect: socket state={:?}, is_open={}", _state, is_open);
            if is_open {
                return -106; // EISCONN
            }

            // Get local endpoint - match address family with remote
            let is_loopback = match endpoint.addr {
                IpAddress::Ipv4(addr) => addr.octets()[0] == 127,
                #[cfg(feature = "proto-ipv6")]
                IpAddress::Ipv6(addr) => addr.is_loopback(),
                #[cfg(not(feature = "proto-ipv6"))]
                _ => false,
            };

            let mut local_endpoint = network_stack()
                .socket_local_endpoint(handle)
                .unwrap_or_else(|| {
                    // Choose local address based on remote address.
                    use smoltcp::wire::IpAddress;
                    let local_addr = match endpoint.addr {
                        IpAddress::Ipv4(_) => {
                            if is_loopback {
                                IpAddress::Ipv4(Ipv4Address::LOCALHOST)
                            } else {
                                IpAddress::Ipv4(Ipv4Address::new(10, 0, 2, 15))
                            }
                        }
                        #[cfg(feature = "proto-ipv6")]
                        IpAddress::Ipv6(_) => {
                            use smoltcp::wire::Ipv6Address;
                            if is_loopback {
                                IpAddress::Ipv6(Ipv6Address::LOCALHOST)
                            } else {
                                IpAddress::Ipv6(Ipv6Address::UNSPECIFIED)
                            }
                        }
                        #[cfg(not(feature = "proto-ipv6"))]
                        _ => IpAddress::Ipv4(Ipv4Address::new(10, 0, 2, 15)),
                    };
                    IpEndpoint::new(local_addr, 0)
                });

            // Allocate ephemeral port if needed
            if local_endpoint.port == 0 {
                // Simple ephemeral port allocation (49152-65535)
                local_endpoint.port = alloc_ephemeral_port();
            }

            pr_debug!("connect: local_endpoint={}", local_endpoint);
            // Persist local endpoint for getsockname() even if smoltcp doesn't expose it yet.
            if let Some(sf) = file
                .as_any()
                .downcast_ref::<crate::net::socket::SocketFile>()
            {
                sf.set_local_endpoint(local_endpoint);
            }
            use crate::net::socket::tcp_connect;
            if let Err(e) = tcp_connect(h, endpoint, local_endpoint) {
                pr_debug!("connect: tcp_connect failed: {:?}", e);
                return e.to_errno();
            }

            // For blocking sockets, wait until connection is established
            if !is_nonblock {
                pr_debug!("connect: handle={:?}, entering wait loop", h);
                loop {
                    // Poll until all loopback packets are processed
                    if is_loopback {
                        crate::net::socket::poll_until_empty();
                    }

                    let state = network_stack()
                        .tcp_connection_state(h)
                        .unwrap_or(TcpConnectionState::Closed);
                    pr_debug!("connect: loop, handle={:?}, state={:?}", h, state);

                    if state == TcpConnectionState::Established {
                        pr_debug!("connect: established");
                        break;
                    }
                    if state == TcpConnectionState::Closed {
                        pr_debug!("connect: socket closed, returning ECONNREFUSED");
                        return -111; // ECONNREFUSED
                    }

                    crate::kernel::yield_task();
                    if crate::ipc::signal_interrupts_syscall(&task) {
                        return -(crate::uapi::errno::EINTR as isize);
                    }
                }
            }

            pr_debug!("connect: tcp_connect success, nonblock={}", is_nonblock);
            crate::pr_info!(
                "[TCP] Connection established: {} -> {}",
                local_endpoint,
                endpoint
            );

            if is_nonblock {
                return -115; // EINPROGRESS
            }
        }
        SocketHandle::Udp(h) => {
            pr_debug!("connect: sockfd={} UDP", sockfd);

            // Ensure this fd is attached to the shared per-port UDP socket.
            // If not yet bound, implicitly bind to an ephemeral port (49152-65535).
            let local_port = match file
                .as_any()
                .downcast_ref::<SocketFile>()
                .and_then(|sf| sf.get_local_endpoint())
            {
                Some(ep) if ep.port != 0 => ep.port,
                _ => alloc_ephemeral_port(),
            };

            if let Some(sf) = file.as_any().downcast_ref::<SocketFile>() {
                // IMPORTANT: use a concrete local source address for loopback, otherwise smoltcp
                // will emit packets with src=0.0.0.0 and iperf3 UDP server will "connect()" to
                // 127.0.0.1 and then drop subsequent datagrams (remote endpoint mismatch).
                let local_addr = match endpoint.addr {
                    IpAddress::Ipv4(a) if a.octets()[0] == 127 => {
                        IpAddress::Ipv4(Ipv4Address::LOCALHOST)
                    }
                    #[cfg(feature = "proto-ipv6")]
                    IpAddress::Ipv6(a) if a.is_loopback() => {
                        use smoltcp::wire::Ipv6Address;
                        IpAddress::Ipv6(Ipv6Address::LOCALHOST)
                    }
                    _ => IpAddress::Ipv4(Ipv4Address::UNSPECIFIED),
                };
                sf.set_local_endpoint(IpEndpoint::new(local_addr, local_port));
            }

            let bind_addr = match endpoint.addr {
                IpAddress::Ipv4(a) if a.octets()[0] == 127 => {
                    Some(IpAddress::Ipv4(Ipv4Address::LOCALHOST))
                }
                #[cfg(feature = "proto-ipv6")]
                IpAddress::Ipv6(a) if a.is_loopback() => {
                    use smoltcp::wire::Ipv6Address;
                    Some(IpAddress::Ipv6(Ipv6Address::LOCALHOST))
                }
                _ => None,
            };

            if let Err(e) = crate::net::socket::udp_attach_fd_to_port(
                tid,
                sockfd as usize,
                &file,
                h,
                local_port,
                bind_addr,
            ) {
                return e.to_errno();
            }
            pr_debug!("connect: sockfd={} UDP -> success", sockfd);
        }
    }

    pr_debug!("connect: sockfd={} -> success", sockfd);
    0
}

/// 发送数据
pub fn send(sockfd: i32, buf: *const u8, len: usize, _flags: i32) -> isize {
    loop {
        let task = current_task();
        let (_tid, file) = {
            let task_lock = task.lock();
            let tid = task_lock.tid;
            if sockfd < 0 {
                pr_debug!("send: EBADF tid={}, sockfd={}", tid, sockfd);
                return -(crate::uapi::errno::EBADF as isize);
            }
            let file = match task_lock.fd_table.get(sockfd as usize) {
                Ok(f) => f,
                Err(_) => {
                    pr_debug!("send: EBADF tid={}, sockfd={}", tid, sockfd);
                    return -(crate::uapi::errno::EBADF as isize);
                }
            };
            (tid, file)
        };

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
            file.write(&kernel_buf)
        };

        match result {
            Ok(n) => {
                pr_debug!("send: sockfd={}, len={} -> sent={}", sockfd, len, n);
                return n as isize;
            }
            Err(e) => {
                pr_debug!("send: sockfd={}, len={} -> error={:?}", sockfd, len, e);
                if e == crate::vfs::FsError::WouldBlock
                    && let Some(socket_file) = file.as_any().downcast_ref::<SocketFile>()
                    && !socket_file.flags().contains(OpenFlags::O_NONBLOCK)
                {
                    drop(file);
                    crate::net::socket::poll_network_and_dispatch();
                    crate::kernel::yield_task();
                    if crate::ipc::signal_interrupts_syscall(&task) {
                        return -(crate::uapi::errno::EINTR as isize);
                    }
                    continue;
                }
                return e.to_errno();
            }
        }
    }
}

/// 接收数据
pub fn recv(sockfd: i32, buf: *mut u8, len: usize, _flags: i32) -> isize {
    loop {
        let task = current_task();
        let (_tid, file) = {
            let task_lock = task.lock();
            let tid = task_lock.tid;
            if sockfd < 0 {
                pr_debug!("recv: EBADF tid={}, sockfd={}", tid, sockfd);
                return -(crate::uapi::errno::EBADF as isize);
            }
            let file = match task_lock.fd_table.get(sockfd as usize) {
                Ok(f) => f,
                Err(_) => {
                    pr_debug!("recv: EBADF tid={}, sockfd={}", tid, sockfd);
                    return -(crate::uapi::errno::EBADF as isize);
                }
            };
            (tid, file)
        };

        let result = {
            let mut kernel_buf = alloc::vec![0u8; len];
            match file.read(&mut kernel_buf) {
                Ok(n) => {
                    unsafe {
                        crate::arch::ArchImpl::copy_to_user(
                            kernel_buf.as_ptr(),
                            crate::arch::address::UA::from_usize(buf as usize),
                            n,
                        )
                        .ok();
                    }
                    Ok(n)
                }
                Err(e) => Err(e),
            }
        };

        match result {
            Ok(n) => {
                pr_debug!("recv: sockfd={}, len={} -> received={}", sockfd, len, n);
                return n as isize;
            }
            Err(e) => {
                pr_debug!("recv: sockfd={}, len={} -> error={:?}", sockfd, len, e);
                if e == crate::vfs::FsError::WouldBlock
                    && let Some(socket_file) = file.as_any().downcast_ref::<SocketFile>()
                    && !socket_file.flags().contains(OpenFlags::O_NONBLOCK)
                {
                    drop(file);
                    crate::net::socket::poll_network_and_dispatch();
                    crate::kernel::yield_task();
                    if crate::ipc::signal_interrupts_syscall(&task) {
                        return -(crate::uapi::errno::EINTR as isize);
                    }
                    continue;
                }
                return e.to_errno();
            }
        }
    }
}

/// 关闭套接字
pub fn close_sock(sockfd: i32) -> isize {
    let task = current_task();
    let task_lock = task.lock();
    let tid = task_lock.tid;

    unregister_socket_fd(tid as usize, sockfd as usize);

    match task_lock.fd_table.close(sockfd as usize) {
        Ok(_) => 0,
        Err(_) => -9,
    }
}
