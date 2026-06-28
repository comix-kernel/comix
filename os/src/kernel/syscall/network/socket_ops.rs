use super::*;

const TCP_LISTENER_POOL_LIMIT: usize = 16;
const AF_INET_U16: u16 = 2;

fn is_local_bind_address(addr: IpAddress) -> bool {
    match addr {
        IpAddress::Ipv4(addr) => {
            addr.is_unspecified()
                || addr.octets()[0] == 127
                || NETWORK_INTERFACE_MANAGER
                    .lock()
                    .get_interfaces()
                    .iter()
                    .any(|iface| {
                        iface
                            .ip_addresses()
                            .iter()
                            .any(|cidr| match cidr.address() {
                                IpAddress::Ipv4(local) => local == addr,
                                #[cfg(feature = "proto-ipv6")]
                                IpAddress::Ipv6(_) => false,
                                #[cfg(not(feature = "proto-ipv6"))]
                                _ => false,
                            })
                    })
        }
        #[cfg(feature = "proto-ipv6")]
        IpAddress::Ipv6(addr) => addr.is_unspecified() || addr.is_loopback(),
        #[cfg(not(feature = "proto-ipv6"))]
        _ => false,
    }
}

/// 创建套接字
pub fn socket(domain: i32, socket_type: i32, _protocol: i32) -> isize {
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

    if domain == AF_UNIX {
        let socket_file = match create_unix_socket(base_type, open_flags) {
            Ok(file) => file,
            Err(e) => return e,
        };
        let task = current_task();
        let task_lock = task.lock();
        return match task_lock.fd_table.alloc_with_flags(socket_file, fd_flags) {
            Ok(fd) => fd as isize,
            Err(e) => e.to_errno(),
        };
    }

    if domain != 2 {
        return -97;
    } // EAFNOSUPPORT

    let handle = match base_type {
        SOCK_STREAM => match create_tcp_socket() {
            Ok(h) => h,
            Err(e) => return e.to_errno(),
        },
        SOCK_DGRAM => match create_udp_socket() {
            Ok(h) => h,
            Err(e) => return e.to_errno(),
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
            pr_debug!(
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

/// 创建一对已连接的套接字
pub fn socketpair(domain: i32, socket_type: i32, _protocol: i32, sv: *mut i32) -> isize {
    if domain != AF_UNIX {
        return -97; // EAFNOSUPPORT
    }

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

    let (left, right) = match create_unix_socket_pair(base_type, open_flags) {
        Ok(pair) => pair,
        Err(e) => return e,
    };

    let task = current_task();
    let task_lock = task.lock();
    let left_fd = match task_lock.fd_table.alloc_with_flags(left, fd_flags) {
        Ok(fd) => fd,
        Err(e) => return e.to_errno(),
    };
    let right_fd = match task_lock.fd_table.alloc_with_flags(right, fd_flags) {
        Ok(fd) => fd,
        Err(e) => {
            let _ = task_lock.fd_table.close(left_fd);
            return e.to_errno();
        }
    };

    if let Err(e) = write_socketpair_fds(sv, left_fd, right_fd) {
        let _ = task_lock.fd_table.close(left_fd);
        let _ = task_lock.fd_table.close(right_fd);
        return e;
    }

    0
}

/// 绑定套接字
pub fn bind(sockfd: i32, addr: *const u8, addrlen: u32) -> isize {
    let task = current_task();
    let task_lock = task.lock();
    let file = match task_lock.fd_table.get(sockfd as usize) {
        Ok(f) => f,
        Err(_) => return -9, // EBADF
    };
    let is_unix_socket = file.as_any().is::<UnixSocketFile>();
    if is_unix_socket {
        let unix_addr = match parse_sockaddr_un(addr, addrlen) {
            Ok(addr) => addr,
            Err(e) => return e,
        };
        drop(task_lock);
        let unix_socket = match file.as_any().downcast_ref::<UnixSocketFile>() {
            Some(socket) => socket,
            None => return -88, // ENOTSOCK
        };
        return unix_socket.bind(unix_addr);
    }

    let family = match read_sockaddr_family(addr, addrlen) {
        Ok(family) => family,
        Err(e) => return e.to_errno(),
    };
    if family != AF_INET_U16 {
        return -(crate::uapi::errno::EAFNOSUPPORT as isize);
    }

    let endpoint = match parse_sockaddr_in(addr, addrlen) {
        Ok(e) => e,
        Err(e) => return e.to_errno(),
    };
    if !is_local_bind_address(endpoint.addr) {
        return -(crate::uapi::errno::EADDRNOTAVAIL as isize);
    }

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

            if let Err(e) = set_socket_local_endpoint(&file, endpoint) {
                return e.to_errno();
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

            if let Err(e) = crate::net::socket::udp_attach_fd_to_port(
                tid,
                sockfd as usize,
                &file,
                h,
                endpoint.port,
                bind_addr,
            ) {
                return e.to_errno();
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

    let file = match task_lock.fd_table.get(sockfd as usize) {
        Ok(f) => f,
        Err(_) => return -9, // EBADF
    };
    if let Some(unix_socket) = file.as_any().downcast_ref::<UnixSocketFile>() {
        return unix_socket.listen(backlog);
    }

    let handle = match get_socket_handle(tid, sockfd as usize) {
        Some(h) => h,
        None => return -88, // ENOTSOCK
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
            let listen_endpoint = loop {
                if endpoint.port == 0 {
                    endpoint.port = alloc_ephemeral_port();
                }

                // Persist so that getsockname() can report the chosen port even if smoltcp
                // doesn't expose it until after listen/connect.
                socket_file.set_local_endpoint(endpoint);

                // Convert endpoint to listen endpoint
                // If bound to 0.0.0.0 or ::, listen on all addresses (addr = None)
                use smoltcp::wire::IpListenEndpoint;
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

                if let Err(e) = network_stack().tcp_listen(h, listen_endpoint) {
                    attempts_left = attempts_left.saturating_sub(1);
                    if attempts_left == 0 {
                        return e.to_errno();
                    }
                    endpoint.port = 0;
                    continue;
                }
                break listen_endpoint;
            };

            socket_file.set_listener(true);
            socket_file.clear_listen_sockets();
            // iperf 会传入非常大的 backlog（甚至 INT_MAX），这里做一个上限避免内存/逻辑风险
            let backlog = (backlog as usize).clamp(1, 128);
            socket_file.set_listen_backlog(backlog);
            if let Err(e) = replenish_tcp_listeners(socket_file, listen_endpoint, backlog) {
                return e;
            }
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

    if let Some(unix_socket) = file.as_any().downcast_ref::<UnixSocketFile>() {
        let is_nonblock = unix_socket.flags().contains(OpenFlags::O_NONBLOCK);
        loop {
            match unix_socket.accept() {
                Ok(conn) => {
                    let peer_addr = conn.peer_addr();
                    if let Err(e) = write_sockaddr_un(addr, addrlen, peer_addr) {
                        return e;
                    }
                    return match task.lock().fd_table.alloc(conn) {
                        Ok(fd) => fd as isize,
                        Err(e) => e.to_errno(),
                    };
                }
                Err(e) if e == -(crate::uapi::errno::EAGAIN as isize) && !is_nonblock => {
                    crate::kernel::yield_task();
                    if crate::ipc::signal_interrupts_syscall(&task) {
                        return -(crate::uapi::errno::EINTR as isize);
                    }
                    continue;
                }
                Err(e) => return e,
            }
        }
    }

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

        if state != TcpListenState::Listen {
            if let Err(e) = replenish_tcp_listeners(socket_file, listen_endpoint, backlog) {
                return e;
            }

            let Some(new_listen_handle) =
                network_stack().take_spare_tcp_listener(socket_file, listen_endpoint)
            else {
                return -11; // EAGAIN
            };

            use crate::net::socket::{update_socket_file_handle, update_socket_handle};
            update_socket_handle(
                tid as usize,
                sockfd as usize,
                SocketHandle::Tcp(new_listen_handle),
            );
            if let Err(e) = update_socket_file_handle(&file, SocketHandle::Tcp(new_listen_handle)) {
                return e.to_errno();
            }

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
            if !socket_file.has_listen_socket(SocketHandle::Tcp(listen_handle)) {
                socket_file.add_listen_socket(SocketHandle::Tcp(listen_handle));
            }
            if let Err(e) = replenish_tcp_listeners(socket_file, listen_endpoint, backlog) {
                return e;
            }
            continue;
        }

        if let Err(e) = replenish_tcp_listeners(socket_file, listen_endpoint, backlog) {
            return e;
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

fn replenish_tcp_listeners(
    socket_file: &SocketFile,
    listen_endpoint: smoltcp::wire::IpListenEndpoint,
    backlog: usize,
) -> Result<(), isize> {
    let target = backlog.clamp(1, TCP_LISTENER_POOL_LIMIT);
    while network_stack().tcp_spare_listener_count(socket_file, listen_endpoint) < target {
        let new_listen_handle = match create_tcp_socket() {
            Ok(SocketHandle::Tcp(h)) => h,
            Err(e) => {
                if network_stack().tcp_spare_listener_count(socket_file, listen_endpoint) > 0 {
                    break;
                }
                return Err(e.to_errno());
            }
            Ok(SocketHandle::Udp(_)) => return Err(-(crate::uapi::errno::EINVAL as isize)),
        };

        if let Err(e) = network_stack().tcp_listen(new_listen_handle, listen_endpoint) {
            network_stack().remove_tcp_socket(new_listen_handle);
            if network_stack().tcp_spare_listener_count(socket_file, listen_endpoint) > 0 {
                break;
            }
            return Err(e.to_errno());
        }

        socket_file.add_listen_socket(SocketHandle::Tcp(new_listen_handle));
    }
    Ok(())
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
