use super::*;

// 设置网络接口配置
pub fn setsockopt(sockfd: i32, level: i32, optname: i32, optval: *const u8, optlen: u32) -> isize {
    use crate::uapi::errno::{EBADF, EINVAL, ENOPROTOOPT, ENOTSOCK};
    use crate::uapi::socket::*;

    if sockfd < 0 || optval.is_null() {
        return -(EINVAL as isize);
    }

    let task = current_task();
    let file = match task.lock().fd_table.get(sockfd as usize) {
        Ok(f) => f,
        Err(_) => return -(EBADF as isize),
    };

    enum SocketOptionTarget<'a> {
        Inet(&'a crate::net::socket::SocketFile),
        Unix(&'a crate::net::unix_socket::UnixSocketFile),
    }

    impl SocketOptionTarget<'_> {
        fn get(&self) -> crate::uapi::socket::SocketOptions {
            match self {
                Self::Inet(socket) => socket.get_socket_options(),
                Self::Unix(socket) => socket.get_socket_options(),
            }
        }

        fn set(&self, options: crate::uapi::socket::SocketOptions) {
            match self {
                Self::Inet(socket) => socket.set_socket_options(options),
                Self::Unix(socket) => socket.set_socket_options(options),
            }
        }
    }

    let target = if let Some(sf) = file
        .as_any()
        .downcast_ref::<crate::net::socket::SocketFile>()
    {
        SocketOptionTarget::Inet(sf)
    } else if let Some(sf) = file
        .as_any()
        .downcast_ref::<crate::net::unix_socket::UnixSocketFile>()
    {
        SocketOptionTarget::Unix(sf)
    } else {
        return -(ENOTSOCK as isize);
    };

    let mut opts = target.get();

    match level {
        SOL_SOCKET => match optname {
            SO_REUSEADDR => set_sockopt_bool!(optval, optlen, opts.reuse_addr),
            SO_REUSEPORT => set_sockopt_bool!(optval, optlen, opts.reuse_port),
            SO_KEEPALIVE => set_sockopt_bool!(optval, optlen, opts.keepalive),
            SO_DONTROUTE | SO_BROADCAST | SO_OOBINLINE => { /* ignore */ }
            SO_SNDBUF => set_sockopt_int!(optval, optlen, opts.send_buffer_size),
            SO_RCVBUF => set_sockopt_int!(optval, optlen, opts.recv_buffer_size),
            SO_RCVLOWAT | SO_SNDLOWAT => { /* ignore */ }
            SO_RCVTIMEO_OLD | SO_SNDTIMEO_OLD => { /* Ignore timeout options */ }
            _ => return -(ENOPROTOOPT as isize),
        },
        _ if matches!(target, SocketOptionTarget::Unix(_)) => return -(ENOPROTOOPT as isize),
        IPPROTO_IP => match optname {
            IP_TOS | IP_TTL | IP_PKTINFO | IP_MTU_DISCOVER | IP_RECVERR => { /* ignore */ }
            _ => return -(ENOPROTOOPT as isize),
        },
        IPPROTO_TCP => match optname {
            TCP_NODELAY => set_sockopt_bool!(optval, optlen, opts.tcp_nodelay),
            _ => return -(ENOPROTOOPT as isize),
        },
        IPPROTO_IPV6 => match optname {
            IPV6_V6ONLY => set_sockopt_bool!(optval, optlen, opts.ipv6_v6only),
            _ => return -(ENOPROTOOPT as isize),
        },
        _ => return -(ENOPROTOOPT as isize),
    }

    target.set(opts);
    0
}

// 获取网络接口配置
pub fn getsockopt(
    sockfd: i32,
    level: i32,
    optname: i32,
    optval: *mut u8,
    optlen: *mut u32,
) -> isize {
    use crate::uapi::errno::{EBADF, EINVAL, ENOPROTOOPT, ENOTSOCK};
    use crate::uapi::socket::*;

    if sockfd < 0 || optval.is_null() || optlen.is_null() {
        return -(EINVAL as isize);
    }

    let task = current_task();
    let file = match task.lock().fd_table.get(sockfd as usize) {
        Ok(f) => f,
        Err(_) => return -(EBADF as isize),
    };

    let opts = if let Some(sf) = file
        .as_any()
        .downcast_ref::<crate::net::socket::SocketFile>()
    {
        sf.get_socket_options()
    } else if let Some(sf) = file
        .as_any()
        .downcast_ref::<crate::net::unix_socket::UnixSocketFile>()
    {
        sf.get_socket_options()
    } else {
        return -(ENOTSOCK as isize);
    };

    if file
        .as_any()
        .downcast_ref::<crate::net::unix_socket::UnixSocketFile>()
        .is_some()
        && level != SOL_SOCKET
    {
        return -(ENOPROTOOPT as isize);
    };

    let available_len = read_from_user(optlen as *const u32) as usize;
    let mut written_len = 0usize;

    match level {
        SOL_SOCKET => match optname {
            SO_REUSEADDR => {
                get_sockopt_bool!(optval, available_len, opts.reuse_addr, written_len)
            }
            SO_REUSEPORT => {
                get_sockopt_bool!(optval, available_len, opts.reuse_port, written_len)
            }
            SO_KEEPALIVE => {
                get_sockopt_bool!(optval, available_len, opts.keepalive, written_len)
            }
            SO_SNDBUF => {
                get_sockopt_int!(optval, available_len, opts.send_buffer_size, written_len)
            }
            SO_RCVBUF => {
                get_sockopt_int!(optval, available_len, opts.recv_buffer_size, written_len)
            }
            _ => return -(ENOPROTOOPT as isize),
        },
        IPPROTO_TCP => match optname {
            TCP_NODELAY => {
                get_sockopt_bool!(optval, available_len, opts.tcp_nodelay, written_len)
            }
            TCP_MAXSEG => {
                get_sockopt_int!(optval, available_len, opts.tcp_maxseg, written_len)
            }
            TCP_CONGESTION => {
                // Return a dummy congestion control name. iperf3 mainly uses this for display.
                let cc = b"cubic\0";
                let n = core::cmp::min(available_len, cc.len());
                unsafe {
                    crate::arch::ArchImpl::copy_to_user(
                        cc.as_ptr(),
                        crate::arch::address::UA::from_usize(optval as usize),
                        n,
                    )
                    .ok();
                }
                written_len = n;
            }
            TCP_INFO => {
                // Best-effort placeholder. smoltcp doesn't currently expose full tcp_info metrics.
                let info = TcpInfo::dummy_established();
                let src = &info as *const TcpInfo as *const u8;
                let n = core::cmp::min(available_len, core::mem::size_of::<TcpInfo>());
                unsafe {
                    crate::arch::ArchImpl::copy_to_user(
                        src,
                        crate::arch::address::UA::from_usize(optval as usize),
                        n,
                    )
                    .ok();
                }
                written_len = n;
            }
            _ => return -(ENOPROTOOPT as isize),
        },
        IPPROTO_IPV6 => match optname {
            IPV6_V6ONLY => {
                get_sockopt_bool!(optval, available_len, opts.ipv6_v6only, written_len)
            }
            _ => return -(ENOPROTOOPT as isize),
        },
        _ => return -(ENOPROTOOPT as isize),
    }

    write_to_user(optlen, written_len as u32);

    0
}
