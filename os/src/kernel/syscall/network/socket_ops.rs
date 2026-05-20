use super::*;

/// 创建套接字
pub fn socket(domain: i32, socket_type: i32, _protocol: i32) -> isize {
    if domain != 2 {
        return -97;
    } // EAFNOSUPPORT

    let base_type = socket_type & SOCK_TYPE_MASK;
    let extra_flags = socket_type & !SOCK_TYPE_MASK;
    let supported_flags = SOCK_NONBLOCK | SOCK_CLOEXEC;
    if extra_flags & !supported_flags != 0 {
        return -22; // EINVAL
    }

    let mut open_flags = OpenFlags::empty();
    if extra_flags & SOCK_NONBLOCK != 0 {
        open_flags |= OpenFlags::O_NONBLOCK;
    }

    let fd_flags = if extra_flags & SOCK_CLOEXEC != 0 {
        FdFlags::CLOEXEC
    } else {
        FdFlags::empty()
    };

    let handle = match base_type {
        SOCK_STREAM => match create_tcp_socket() {
            Ok(h) => h,
            Err(_) => return -12, // ENOMEM
        },
        SOCK_DGRAM => match create_udp_socket() {
            Ok(h) => h,
            Err(_) => return -12, // ENOMEM
        },
        _ => return -94, // ESOCKTNOSUPPORT
    };

    let socket_file = Arc::new(SocketFile::new_with_flags(handle, open_flags));
    let task = current_task();

    let task_lock = task.lock();
    let tid = task_lock.tid;
    match task_lock.fd_table.alloc_with_flags(socket_file, fd_flags) {
        Ok(fd) => {
            register_socket_fd(task_lock.tid as usize, fd, handle);
            let handle_type = match handle {
                SocketHandle::Tcp(_) => "TCP",
                SocketHandle::Udp(_) => "UDP",
            };
            pr_info!(
                "[SOCKET] Created {} socket: tid={}, fd={}, domain={}, type={}",
                handle_type,
                tid,
                fd,
                domain,
                base_type
            );
            fd as isize
        }
        Err(_) => -24, // EMFILE
    }
}

/// 绑定套接字
pub fn bind(sockfd: i32, addr: *const u8, addrlen: u32) -> isize {
    let endpoint = match parse_sockaddr_in(addr, addrlen) {
        Ok(e) => e,
        Err(_) => return -22, // EINVAL
    };

    let task = current_task();
    let task_lock = task.lock();
    let tid = task_lock.tid as usize;

    pr_debug!(
        "bind: tid={}, sockfd={}, endpoint={}",
        tid,
        sockfd,
        endpoint
    );

    let handle = match get_socket_handle(tid, sockfd as usize) {
        Some(h) => h,
        None => return -88, // ENOTSOCK
    };

    let file = match task_lock.fd_table.get(sockfd as usize) {
        Ok(f) => f,
        Err(_) => return -9, // EBADF
    };
    drop(task_lock);

    // For TCP: just save the endpoint, listen() will call smoltcp's listen()
    // For UDP: bind immediately
    match handle {
        SocketHandle::Tcp(_) => {
            use crate::net::socket::set_socket_local_endpoint;
            // Linux behavior: bind(..., port=0) asks the kernel to choose an ephemeral port.
            let endpoint = if endpoint.port == 0 {
                IpEndpoint::new(endpoint.addr, alloc_ephemeral_port())
            } else {
                endpoint
            };

            // Linux behavior: binding an already-bound TCP socket is invalid.
            if let Some(sf) = file.as_any().downcast_ref::<SocketFile>()
                && let Some(old) = sf.get_local_endpoint()
                && old.port != 0
            {
                return -22; // EINVAL
            }

            if set_socket_local_endpoint(&file, endpoint).is_err() {
                return -22; // EINVAL
            }
        }
        SocketHandle::Udp(h) => {
            // Linux allows binding multiple UDP sockets to the same port (with SO_REUSEADDR).
            // smoltcp's UDP demux only matches by dst_port, so we implement a per-port dispatcher:
            // one smoltcp UDP socket per local port, and per-fd queues filtered by remote endpoint.

            // Linux behavior: binding an already-bound UDP socket is invalid.
            if let Some(sf) = file.as_any().downcast_ref::<SocketFile>()
                && let Some(old) = sf.get_local_endpoint()
                && old.port != 0
            {
                return -22; // EINVAL
            }

            // Bind port 0 => allocate an ephemeral port.
            let endpoint = if endpoint.port == 0 {
                IpEndpoint::new(endpoint.addr, alloc_ephemeral_port())
            } else {
                endpoint
            };

            // Persist local endpoint on the SocketFile for dispatch matching.
            if let Some(sf) = file.as_any().downcast_ref::<SocketFile>() {
                sf.set_local_endpoint(endpoint);
            }

            let bind_addr = match endpoint.addr {
                IpAddress::Ipv4(a) if a.is_unspecified() => None,
                IpAddress::Ipv4(_) => Some(endpoint.addr),
                #[cfg(feature = "proto-ipv6")]
                IpAddress::Ipv6(a) if a.is_unspecified() => None,
                #[cfg(feature = "proto-ipv6")]
                IpAddress::Ipv6(_) => Some(endpoint.addr),
                #[cfg(not(feature = "proto-ipv6"))]
                _ => None,
            };

            if crate::net::socket::udp_attach_fd_to_port(
                tid,
                sockfd as usize,
                &file,
                h,
                endpoint.port,
                bind_addr,
            )
            .is_err()
            {
                return -98; // EADDRINUSE / bind error
            }
        }
    }

    0
}

/// 监听连接
pub fn listen(sockfd: i32, backlog: i32) -> isize {
    if backlog < 0 {
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

    match handle {
        SocketHandle::Tcp(h) => {
            use crate::net::socket::SocketFile;
            let socket_file = match file.as_any().downcast_ref::<SocketFile>() {
                Some(sf) => sf,
                None => return -88, // ENOTSOCK
            };

            // Linux behavior: listen() on an unbound TCP socket implicitly binds it to
            // INADDR_ANY:ephemeral. Also, if the socket was bound with port=0, the kernel must
            // choose a non-zero port before starting to listen.
            use crate::net::socket::get_socket_local_endpoint;
            let mut endpoint = get_socket_local_endpoint(&file)
                .unwrap_or_else(|| IpEndpoint::new(IpAddress::Ipv4(Ipv4Address::UNSPECIFIED), 0));

            pr_debug!(
                "listen: tid={}, sockfd={}, endpoint(before)={}",
                tid,
                sockfd,
                endpoint
            );

            let mut attempts_left: usize = if endpoint.port == 0 { 32 } else { 1 };
            loop {
                if endpoint.port == 0 {
                    endpoint.port = alloc_ephemeral_port();
                }

                // Persist so that getsockname() can report the chosen port even if smoltcp
                // doesn't expose it until after listen/connect.
                socket_file.set_local_endpoint(endpoint);

                // Convert endpoint to listen endpoint
                // If bound to 0.0.0.0 or ::, listen on all addresses (addr = None)
                use smoltcp::wire::{IpAddress, IpListenEndpoint};
                let listen_endpoint = match endpoint.addr {
                    IpAddress::Ipv4(addr) if addr.is_unspecified() => IpListenEndpoint {
                        addr: None,
                        port: endpoint.port,
                    },
                    IpAddress::Ipv6(addr) if addr.is_unspecified() => IpListenEndpoint {
                        addr: None,
                        port: endpoint.port,
                    },
                    _ => IpListenEndpoint {
                        addr: Some(endpoint.addr),
                        port: endpoint.port,
                    },
                };

                pr_debug!(
                    "listen: converted endpoint={} to listen_endpoint addr={:?} port={}",
                    endpoint,
                    listen_endpoint.addr,
                    listen_endpoint.port
                );

                if network_stack().tcp_listen(h, listen_endpoint).is_err() {
                    attempts_left = attempts_left.saturating_sub(1);
                    if attempts_left == 0 {
                        return -98; // EADDRINUSE
                    }
                    endpoint.port = 0;
                    continue;
                }
                break;
            }

            socket_file.set_listener(true);
            socket_file.clear_listen_sockets();
            // iperf 会传入非常大的 backlog（甚至 INT_MAX），这里做一个上限避免内存/逻辑风险
            let backlog = (backlog as usize).clamp(1, 128);
            socket_file.set_listen_backlog(backlog);
            0
        }
        SocketHandle::Udp(_) => {
            -95 // EOPNOTSUPP - UDP doesn't support listen
        }
    }
}

/// 接受连接
pub fn accept(sockfd: i32, addr: *mut u8, addrlen: *mut u32) -> isize {
    let task = current_task();
    let tid = task.lock().tid;

    let file = match task.lock().fd_table.get(sockfd as usize) {
        Ok(f) => f,
        Err(_) => return -9, // EBADF
    };

    use crate::net::socket::SocketFile;
    let socket_file = match file.as_any().downcast_ref::<SocketFile>() {
        Some(sf) => sf,
        None => return -88, // ENOTSOCK
    };

    if !socket_file.is_listener() {
        return -22; // EINVAL - not a listening socket
    }

    // Check if socket is non-blocking
    let is_nonblock = socket_file
        .flags()
        .contains(crate::uapi::fcntl::OpenFlags::O_NONBLOCK);
    let backlog = socket_file.listen_backlog().clamp(1, 128);

    loop {
        // 推进 loopback + 网络状态机
        crate::net::socket::poll_until_empty();

        // 1) 先从“已排队连接”中取一个已完成握手的连接
        if let Some(SocketHandle::Tcp(conn_handle)) =
            socket_file.take_established_from_listen_queue()
        {
            return accept_return_conn(task.clone(), tid as usize, conn_handle, addr, addrlen);
        }

        // 2) 检查当前监听 handle 是否已经进入握手/已建立状态；
        //    如果进入了（state != Listen），就立刻创建一个新的 listener 继续监听，
        //    把旧 handle 放入队列（Established 直接返回，SynReceived 等待后续成熟）。
        let listen_handle = match get_socket_handle(tid as usize, sockfd as usize) {
            Some(SocketHandle::Tcp(h)) => h,
            Some(SocketHandle::Udp(_)) => return -95, // EOPNOTSUPP
            None => return -88,                       // ENOTSOCK
        };

        let (state, listen_endpoint) =
            match network_stack().tcp_listener_state_endpoint(listen_handle) {
                Some(v) => v,
                None => return -88, // ENOTSOCK
            };

        if state != TcpListenState::Listen && socket_file.listen_sockets_len() < backlog {
            // detach current listen socket immediately
            let new_listen_handle = match create_tcp_socket() {
                Ok(SocketHandle::Tcp(h)) => h,
                _ => return -12, // ENOMEM
            };

            if network_stack()
                .tcp_listen(new_listen_handle, listen_endpoint)
                .is_err()
            {
                network_stack().remove_tcp_socket(new_listen_handle);
                return -12; // ENOMEM or other error
            }

            use crate::net::socket::{update_socket_file_handle, update_socket_handle};
            update_socket_handle(
                tid as usize,
                sockfd as usize,
                SocketHandle::Tcp(new_listen_handle),
            );
            update_socket_file_handle(&file, SocketHandle::Tcp(new_listen_handle)).unwrap();

            // Established / CloseWait: this handle is ready to return right away.
            if matches!(
                state,
                TcpListenState::Established | TcpListenState::CloseWait
            ) {
                return accept_return_conn(
                    task.clone(),
                    tid as usize,
                    listen_handle,
                    addr,
                    addrlen,
                );
            }

            // Otherwise: keep it as pending (SynReceived, etc).
            socket_file.add_listen_socket(SocketHandle::Tcp(listen_handle));
            continue;
        }

        if is_nonblock {
            return -11; // EAGAIN
        }
        crate::kernel::yield_task();
        if crate::ipc::signal_interrupts_syscall(&task) {
            return -(crate::uapi::errno::EINTR as isize);
        }
    }
}

fn accept_return_conn(
    task: crate::kernel::SharedTask,
    tid: usize,
    conn_handle: smoltcp::iface::SocketHandle,
    addr: *mut u8,
    addrlen: *mut u32,
) -> isize {
    use crate::net::socket::SocketFile;

    let (remote_endpoint, local_endpoint) = match network_stack().tcp_accept_endpoints(conn_handle)
    {
        Some(v) => v,
        None => return -11, // EAGAIN
    };

    if !addr.is_null() && !addrlen.is_null() {
        // accept(): Linux truncates if addrlen is too small; our helper implements that.
        let _ = write_sockaddr_in(addr, addrlen, remote_endpoint);
    }

    let conn = Arc::new(SocketFile::new(SocketHandle::Tcp(conn_handle)));
    if let Some(local_ep) = local_endpoint {
        conn.set_local_endpoint(local_ep);
    }
    conn.set_remote_endpoint(remote_endpoint);

    match task.lock().fd_table.alloc(conn) {
        Ok(fd) => {
            register_socket_fd(tid, fd, SocketHandle::Tcp(conn_handle));
            fd as isize
        }
        Err(_) => -24, // EMFILE
    }
}
