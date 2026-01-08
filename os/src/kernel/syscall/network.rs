//! 网络相关的系统调用实现

use core::ffi::{CStr, c_char};
use core::sync::atomic::{AtomicU16, Ordering};

/// ifaddrs 结构体布局 - 与 Linux libc 兼容
#[repr(C)]
struct IfAddrs {
    ifa_next: usize,    // 下一个接口的用户虚拟地址
    ifa_name: usize,    // 接口名称的用户虚拟地址
    ifa_flags: u32,     // 接口标志
    ifa_addr: usize,    // 接口地址的用户虚拟地址
    ifa_netmask: usize, // 网络掩码的用户虚拟地址
    ifa_ifu: usize,     // 广播地址或目标地址的用户虚拟地址
    ifa_data: usize,    // 统计数据的用户虚拟地址
}

/// sockaddr_in 结构体 (IPv4)
#[repr(C)]
struct SockAddrIn {
    sin_family: u16,
    sin_port: u16,
    sin_addr: [u8; 4],
    sin_zero: [u8; 8],
}

/// 接口标志 (Linux IFF_* flags)
const IFF_UP: u32 = 1 << 0; // 接口已启用
const IFF_BROADCAST: u32 = 1 << 1; // 支持广播
const IFF_LOOPBACK: u32 = 1 << 3; // 回环接口
const IFF_RUNNING: u32 = 1 << 6; // 接口正在运行
const IFF_MULTICAST: u32 = 1 << 12; // 支持多播

const AF_INET: u16 = 2;

const EPHEMERAL_PORT_START: u16 = 49152;
const EPHEMERAL_PORT_END: u16 = u16::MAX;

static NEXT_EPHEMERAL_PORT: AtomicU16 = AtomicU16::new(EPHEMERAL_PORT_START);

fn alloc_ephemeral_port() -> u16 {
    // Keep the counter within [EPHEMERAL_PORT_START, EPHEMERAL_PORT_END] and avoid wrapping to 0.
    NEXT_EPHEMERAL_PORT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
            Some(if cur == EPHEMERAL_PORT_END {
                EPHEMERAL_PORT_START
            } else {
                cur + 1
            })
        })
        .unwrap_or(EPHEMERAL_PORT_START)
}

macro_rules! set_sockopt_bool {
    ($optval:expr, $optlen:expr, $field:expr) => {
        if $optlen >= 4 {
            let val = *($optval as *const i32);
            $field = val != 0;
        }
    };
}

macro_rules! set_sockopt_int {
    ($optval:expr, $optlen:expr, $field:expr) => {
        if $optlen >= 4 {
            let val = *($optval as *const i32);
            if val < 0 {
                return -(EINVAL as isize);
            }
            // Clamp to reasonable range: min 4KB, max 16MB
            let val = (val as usize).max(4096).min(16 * 1024 * 1024);
            $field = val;
        }
    };
}

macro_rules! get_sockopt_bool {
    ($optval:expr, $avail:expr, $field:expr, $written:expr) => {
        if $avail >= 4 {
            *($optval as *mut i32) = if $field { 1 } else { 0 };
            $written = 4;
        }
    };
}

macro_rules! get_sockopt_int {
    ($optval:expr, $avail:expr, $field:expr, $written:expr) => {
        if $avail >= 4 {
            *($optval as *mut i32) = $field as i32;
            $written = 4;
        }
    };
}

use crate::vfs::File;
use crate::{
    arch::trap::SumGuard,
    kernel::current_task,
    net::{
        config::NetworkConfigManager,
        interface::NETWORK_INTERFACE_MANAGER,
        socket::{
            SOCKET_SET, SocketFile, SocketHandle, create_tcp_socket, create_udp_socket,
            get_socket_handle, parse_sockaddr_in, register_socket_fd, unregister_socket_fd,
            write_sockaddr_in,
        },
    },
    pr_debug, pr_info, println,
    uapi::{
        fcntl::{FdFlags, OpenFlags},
        socket::{SOCK_CLOEXEC, SOCK_DGRAM, SOCK_NONBLOCK, SOCK_STREAM, SOCK_TYPE_MASK},
    },
};
use alloc::sync::Arc;
use smoltcp::socket::{tcp, udp};
use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address};

/// 获取网络接口列表
pub fn get_network_interfaces() -> isize {
    0
}

/// 设置网络接口配置
pub fn set_network_interface_config(
    ifname: *const c_char,
    ip: *const c_char,
    gateway: *const c_char,
    mask: *const c_char,
) -> isize {
    // 解析参数
    unsafe {
        let _guard = SumGuard::new();

        let ifname_str = match get_c_str_safe(ifname) {
            Some(s) => s,
            None => {
                return -1;
            }
        };

        let ip_str = match get_c_str_safe(ip) {
            Some(s) => s,
            None => {
                return -2;
            }
        };

        let gateway_str = match get_c_str_safe(gateway) {
            Some(s) => s,
            None => {
                return -3;
            }
        };

        let mask_str = match get_c_str_safe(mask) {
            Some(s) => s,
            None => {
                return -4;
            }
        };

        // 设置网络配置
        match NetworkConfigManager::set_interface_config(ifname_str, ip_str, gateway_str, mask_str)
        {
            Ok(_) => 0,
            Err(e) => {
                println!("Network config error: {:?}", e);
                -5
            }
        }
    }
}

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

    let mut task_lock = task.lock();
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
    let endpoint = unsafe {
        let _guard = SumGuard::new();
        let ep = parse_sockaddr_in(addr, addrlen);
        match ep {
            Ok(e) => e,
            Err(_) => return -22, // EINVAL
        }
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
            if set_socket_local_endpoint(&file, endpoint).is_err() {
                return -22; // EINVAL
            }
        }
        SocketHandle::Udp(h) => {
            // Linux allows binding multiple UDP sockets to the same port (with SO_REUSEADDR).
            // smoltcp's UDP demux only matches by dst_port, so we implement a per-port dispatcher:
            // one smoltcp UDP socket per local port, and per-fd queues filtered by remote endpoint.

            // Linux behavior: binding an already-bound UDP socket is invalid.
            if let Some(sf) = file.as_any().downcast_ref::<SocketFile>() {
                if let Some(old) = sf.get_local_endpoint() {
                    if old.port != 0 {
                        return -22; // EINVAL
                    }
                }
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
            use crate::net::socket::get_socket_local_endpoint;
            let endpoint = match get_socket_local_endpoint(&file) {
                Some(ep) => ep,
                None => return -22, // EINVAL - must bind first
            };

            pr_debug!(
                "listen: tid={}, sockfd={}, endpoint={}",
                tid,
                sockfd,
                endpoint
            );

            use crate::net::socket::SocketFile;
            let socket_file = match file.as_any().downcast_ref::<SocketFile>() {
                Some(sf) => sf,
                None => return -88, // ENOTSOCK
            };

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

            let mut sockets = SOCKET_SET.lock();
            let socket = sockets.get_mut::<tcp::Socket>(h);
            if socket.listen(listen_endpoint).is_err() {
                return -98; // EADDRINUSE
            }
            drop(sockets);

            socket_file.set_listener(true);
            socket_file.clear_listen_sockets();
            // iperf 会传入非常大的 backlog（甚至 INT_MAX），这里做一个上限避免内存/逻辑风险
            let backlog = (backlog as usize).max(1).min(128);
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
    let backlog = socket_file.listen_backlog().max(1).min(128);

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

        let (state, listen_endpoint) = {
            let sockets = SOCKET_SET.lock();
            let s = sockets.get::<tcp::Socket>(listen_handle);
            (s.state(), s.listen_endpoint())
        };

        if state != tcp::State::Listen && socket_file.listen_sockets_len() < backlog {
            // detach current listen socket immediately
            let new_listen_handle = match create_tcp_socket() {
                Ok(SocketHandle::Tcp(h)) => h,
                _ => return -12, // ENOMEM
            };

            let mut sockets = SOCKET_SET.lock();
            let new_listen_socket = sockets.get_mut::<tcp::Socket>(new_listen_handle);
            if new_listen_socket.listen(listen_endpoint).is_err() {
                sockets.remove(new_listen_handle);
                return -12; // ENOMEM or other error
            }
            drop(sockets);

            use crate::net::socket::{update_socket_file_handle, update_socket_handle};
            update_socket_handle(
                tid as usize,
                sockfd as usize,
                SocketHandle::Tcp(new_listen_handle),
            );
            update_socket_file_handle(&file, SocketHandle::Tcp(new_listen_handle)).unwrap();

            // Established / CloseWait: this handle is ready to return right away.
            if matches!(state, tcp::State::Established | tcp::State::CloseWait) {
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

    let sockets = SOCKET_SET.lock();
    let conn_socket = sockets.get::<tcp::Socket>(conn_handle);
    let remote_endpoint = match conn_socket.remote_endpoint() {
        Some(ep) => ep,
        None => return -11, // EAGAIN
    };

    if !addr.is_null() && !addrlen.is_null() {
        let _guard = SumGuard::new();
        unsafe {
            // accept(): Linux truncates if addrlen is too small; our helper implements that.
            let _ = write_sockaddr_in(addr, addrlen, remote_endpoint);
        }
    }

    let conn = Arc::new(SocketFile::new(SocketHandle::Tcp(conn_handle)));
    if let Some(local_ep) = conn_socket.local_endpoint() {
        conn.set_local_endpoint(local_ep);
    }
    conn.set_remote_endpoint(remote_endpoint);
    drop(sockets);

    match task.lock().fd_table.alloc(conn) {
        Ok(fd) => {
            register_socket_fd(tid, fd, SocketHandle::Tcp(conn_handle));
            fd as isize
        }
        Err(_) => -24, // EMFILE
    }
}

/// 连接到远程地址
pub fn connect(sockfd: i32, addr: *const u8, addrlen: u32) -> isize {
    let endpoint = unsafe {
        let _guard = SumGuard::new();
        let ep = parse_sockaddr_in(addr, addrlen);
        match ep {
            Ok(e) => {
                pr_debug!("connect: sockfd={}, endpoint={}", sockfd, e);
                e
            }
            Err(_) => return -22, // EINVAL
        }
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
    set_socket_remote_endpoint(&file, endpoint).unwrap();

    let is_nonblock = file
        .flags()
        .contains(crate::uapi::fcntl::OpenFlags::O_NONBLOCK);

    match handle {
        SocketHandle::Tcp(h) => {
            let sockets = SOCKET_SET.lock();
            let socket = sockets.get::<tcp::Socket>(h);
            pr_debug!(
                "connect: socket state={:?}, is_open={}",
                socket.state(),
                socket.is_open()
            );
            if socket.is_open() {
                return -106; // EISCONN
            }

            // Get local endpoint - match address family with remote
            let is_loopback = match endpoint.addr {
                IpAddress::Ipv4(addr) => addr.octets()[0] == 127,
                #[cfg(feature = "proto-ipv6")]
                IpAddress::Ipv6(addr) => addr.is_loopback(),
                _ => false,
            };

            let mut local_endpoint = socket.local_endpoint().unwrap_or_else(|| {
                // Choose local address based on remote address
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
                            IpAddress::Ipv6(Ipv6Address::LOOPBACK)
                        } else {
                            // Use unspecified address, let the interface choose
                            IpAddress::Ipv6(Ipv6Address::UNSPECIFIED)
                        }
                    }
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
            drop(sockets);

            use crate::net::socket::tcp_connect;
            if let Err(e) = tcp_connect(h, endpoint, local_endpoint) {
                pr_debug!("connect: tcp_connect failed: {:?}", e);
                return -22; // EINVAL or connection error
            }

            // For blocking sockets, wait until connection is established
            if !is_nonblock {
                use crate::net::socket::SOCKET_SET;
                pr_debug!("connect: handle={:?}, entering wait loop", h);
                loop {
                    // Poll until all loopback packets are processed
                    if is_loopback {
                        crate::net::socket::poll_until_empty();
                    }

                    let sockets = SOCKET_SET.lock();
                    let socket = sockets.get::<smoltcp::socket::tcp::Socket>(h);
                    let state = socket.state();
                    pr_debug!("connect: loop, handle={:?}, state={:?}", h, state);
                    drop(sockets);

                    if state == smoltcp::socket::tcp::State::Established {
                        pr_debug!("connect: established");
                        break;
                    }
                    if state == smoltcp::socket::tcp::State::Closed {
                        pr_debug!("connect: socket closed, returning ECONNREFUSED");
                        return -111; // ECONNREFUSED
                    }

                    crate::kernel::yield_task();
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
                        IpAddress::Ipv6(Ipv6Address::LOOPBACK)
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
                    Some(IpAddress::Ipv6(Ipv6Address::LOOPBACK))
                }
                _ => None,
            };

            if crate::net::socket::udp_attach_fd_to_port(
                tid,
                sockfd as usize,
                &file,
                h,
                local_port,
                bind_addr,
            )
            .is_err()
            {
                return -98;
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
        let file = match task.lock().fd_table.get(sockfd as usize) {
            Ok(f) => f,
            Err(_) => {
                pr_debug!("send: sockfd={} -> EBADF", sockfd);
                return -9;
            }
        };

        let result = {
            let _guard = SumGuard::new();
            let data = unsafe { core::slice::from_raw_parts(buf, len) };
            file.write(data)
        };

        match result {
            Ok(n) => {
                pr_debug!("send: sockfd={}, len={} -> sent={}", sockfd, len, n);
                return n as isize;
            }
            Err(e) => {
                pr_debug!("send: sockfd={}, len={} -> error={:?}", sockfd, len, e);
                if e == crate::vfs::FsError::WouldBlock {
                    if let Some(socket_file) = file.as_any().downcast_ref::<SocketFile>() {
                        if !socket_file.flags().contains(OpenFlags::O_NONBLOCK) {
                            drop(file);
                            drop(task);
                            crate::net::socket::poll_network_and_dispatch();
                            crate::kernel::yield_task();
                            continue;
                        }
                    }
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
        let file = match task.lock().fd_table.get(sockfd as usize) {
            Ok(f) => f,
            Err(_) => {
                pr_debug!("recv: sockfd={} -> EBADF", sockfd);
                return -9;
            }
        };

        let result = {
            let _guard = SumGuard::new();
            let data = unsafe { core::slice::from_raw_parts_mut(buf, len) };
            file.read(data)
        };

        match result {
            Ok(n) => {
                pr_debug!("recv: sockfd={}, len={} -> received={}", sockfd, len, n);
                return n as isize;
            }
            Err(e) => {
                pr_debug!("recv: sockfd={}, len={} -> error={:?}", sockfd, len, e);
                if e == crate::vfs::FsError::WouldBlock {
                    if let Some(socket_file) = file.as_any().downcast_ref::<SocketFile>() {
                        if !socket_file.flags().contains(OpenFlags::O_NONBLOCK) {
                            drop(file);
                            drop(task);
                            crate::net::socket::poll_network_and_dispatch();
                            crate::kernel::yield_task();
                            continue;
                        }
                    }
                }
                return e.to_errno();
            }
        }
    }
}

/// 关闭套接字
pub fn close_sock(sockfd: i32) -> isize {
    let task = current_task();
    let mut task_lock = task.lock();
    let tid = task_lock.tid;

    unregister_socket_fd(tid as usize, sockfd as usize);

    match task_lock.fd_table.close(sockfd as usize) {
        Ok(_) => 0,
        Err(_) => -9,
    }
}

/// 安全地获取C字符串
unsafe fn get_c_str_safe(ptr: *const c_char) -> Option<&'static str> {
    if ptr.is_null() {
        return None;
    }

    match CStr::from_ptr(ptr).to_str() {
        Ok(s) => Some(s),
        Err(_) => None,
    }
}

/// 获取网络接口统计信息
///
/// # 参数
/// - `ifname`: 接口名称的 C 字符串指针
/// - `stats`: 用于存储统计信息的缓冲区（用户态地址）
/// - `size`: 缓冲区大小
///
/// # 返回值
/// - 成功返回 0
/// - 失败返回负的错误码
fn get_interface_stats(ifname: *const c_char, stats: *mut u8, size: usize) -> isize {
    use crate::arch::trap::SumGuard;
    use crate::uapi::errno::{EFAULT, EINVAL, ENODEV};

    if ifname.is_null() || stats.is_null() {
        return -(EFAULT as isize);
    }

    // 最小结构体大小检查
    // struct rtnl_link_stats64 的大小约为 192 字节
    const MIN_STATS_SIZE: usize = 192;
    if size < MIN_STATS_SIZE {
        return -(EINVAL as isize);
    }

    let _guard = SumGuard::new();

    // 解析接口名称
    let if_name_str = match unsafe { get_c_str_safe(ifname) } {
        Some(s) => s,
        None => return -(EINVAL as isize),
    };

    // 查找网络接口
    let iface_manager = NETWORK_INTERFACE_MANAGER.lock();
    let interface = match iface_manager.find_interface_by_name(if_name_str) {
        Some(iface) => iface,
        None => return -(ENODEV as isize),
    };

    // 获取设备统计信息
    let _device = interface.device();

    // 填充统计信息结构 (struct rtnl_link_stats64)
    // 结构体布局（简化版）：
    // offset 0:   rx_packets (u64)
    // offset 8:   tx_packets (u64)
    // offset 16:  rx_bytes (u64)
    // offset 24:  tx_bytes (u64)
    // offset 32:  rx_errors (u64)
    // offset 40:  tx_errors (u64)
    // offset 48:  rx_dropped (u64)
    // offset 56:  tx_dropped (u64)
    // offset 64:  multicast (u64)
    // offset 72:  collisions (u64)
    // ... 更多字段

    unsafe {
        let stats_slice = core::slice::from_raw_parts_mut(stats, size);

        // 清零整个结构
        stats_slice.fill(0);

        // TODO: 当 NetDevice trait 扩展后，从设备获取真实的统计数据
        // 目前返回零值表示统计信息不可用
        //
        // 未来的实现示例：
        // let stats_ptr = stats as *mut u64;
        // *stats_ptr.add(0) = device.get_rx_packets();
        // *stats_ptr.add(1) = device.get_tx_packets();
        // *stats_ptr.add(2) = device.get_rx_bytes();
        // *stats_ptr.add(3) = device.get_tx_bytes();
        // ... 等等
    }

    0 // 成功
}

pub fn init_network_syscalls() {
    println!("Network syscalls initialized");
}

/// 获取网络接口地址列表 (Linux标准系统调用)
///
/// 这个函数会在用户态内存空间分配一块连续内存，存储所有接口信息
/// 包括：ifaddrs 链表、sockaddr 结构、接口名称字符串等
pub fn getifaddrs(ifap: *mut *mut u8) -> isize {
    use crate::arch::trap::SumGuard;
    use crate::uapi::errno::{EFAULT, ENOMEM};

    if ifap.is_null() {
        return -(EFAULT as isize);
    }

    let _guard = SumGuard::new();

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct IfAddrsAllocHeader {
        magic: u64,
        map_len: usize,
    }
    const IFADDRS_ALLOC_MAGIC: u64 = 0x434f4d49585f4946; // "COMIX_IF"
    const IFADDRS_HEADER_SIZE: usize = (core::mem::size_of::<IfAddrsAllocHeader>() + 7) & !7;

    // 获取所有网络接口
    let interfaces = NETWORK_INTERFACE_MANAGER.lock().get_interfaces().to_vec();

    if interfaces.is_empty() {
        unsafe {
            *ifap = core::ptr::null_mut();
        }
        return 0;
    }

    // 计算需要的总内存大小
    // 每个接口需要：
    // - 1 个 IfAddrs 结构体
    // - 1 个 SockAddrIn (addr)
    // - 1 个 SockAddrIn (netmask)
    // - 1 个 SockAddrIn (broadcast，如果需要)
    // - 接口名称字符串 (包括 null 终止符)

    let mut total_size = 0usize;
    let ifaddrs_size = core::mem::size_of::<IfAddrs>();
    let sockaddr_size = core::mem::size_of::<SockAddrIn>();

    for iface in interfaces.iter() {
        total_size += ifaddrs_size; // IfAddrs 结构
        total_size += sockaddr_size * 3; // addr + netmask + broadcast
        total_size += iface.name().len() + 1; // 名称 + '\0'

        // 每个 IP 地址都需要一套完整结构
        let ip_count = iface.ip_addresses().len().max(1);
        total_size += (ifaddrs_size + sockaddr_size * 3) * (ip_count.saturating_sub(1));
    }

    // 添加对齐填充
    total_size = (total_size + 7) & !7;

    // 使用 mmap 在用户空间分配内存
    let (user_mem_start, map_len) = {
        use crate::config::PAGE_SIZE;
        use crate::kernel::syscall::mm::mmap;
        use crate::uapi::mm::{MapFlags, ProtFlags};

        // 额外预留一段 header，用于 freeifaddrs 释放整块映射（Linux ABI 语义）
        let map_len = {
            let raw = total_size + IFADDRS_HEADER_SIZE;
            (raw + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
        };

        let addr = mmap(
            core::ptr::null_mut(), // 让内核选择地址
            map_len,
            (ProtFlags::READ | ProtFlags::WRITE).bits(),
            (MapFlags::ANONYMOUS | MapFlags::PRIVATE).bits(),
            -1, // 匿名映射
            0,
        );

        if addr < 0 || addr == 0 {
            return -(ENOMEM as isize);
        }

        (addr as usize + IFADDRS_HEADER_SIZE, map_len)
    };

    // 写入 header（位于 ifa 链表之前）
    unsafe {
        let header_ptr = (user_mem_start - IFADDRS_HEADER_SIZE) as *mut IfAddrsAllocHeader;
        header_ptr.write(IfAddrsAllocHeader {
            magic: IFADDRS_ALLOC_MAGIC,
            map_len,
        });
    }

    // 现在开始在用户内存中构建扁平化的数据结构
    let mut current_offset = 0usize;
    let mut first_ifaddrs_addr = 0usize;
    let mut prev_ifaddrs_addr = 0usize;

    unsafe {
        for iface in interfaces.iter() {
            let ip_addrs = iface.ip_addresses();
            let ip_list = if ip_addrs.is_empty() {
                // 即使没有 IP，也创建一个条目
                alloc::vec![None]
            } else {
                ip_addrs
                    .iter()
                    .map(|ip| Some(*ip))
                    .collect::<alloc::vec::Vec<_>>()
            };

            for ip_cidr_opt in ip_list.iter() {
                // 1. IfAddrs 结构体位置
                let ifaddrs_addr = user_mem_start + current_offset;
                if first_ifaddrs_addr == 0 {
                    first_ifaddrs_addr = ifaddrs_addr;
                }
                current_offset += ifaddrs_size;

                // 2. sockaddr (addr) 位置
                let addr_addr = user_mem_start + current_offset;
                current_offset += sockaddr_size;

                // 3. sockaddr (netmask) 位置
                let netmask_addr = user_mem_start + current_offset;
                current_offset += sockaddr_size;

                // 4. sockaddr (broadcast) 位置
                let broadcast_addr = user_mem_start + current_offset;
                current_offset += sockaddr_size;

                // 5. 接口名称位置
                let name_addr = user_mem_start + current_offset;
                let name_bytes = iface.name().as_bytes();
                current_offset += name_bytes.len() + 1; // +1 for null terminator

                // 8字节对齐
                current_offset = (current_offset + 7) & !7;

                // 填充 IfAddrs 结构体
                let ifaddrs_ptr = ifaddrs_addr as *mut IfAddrs;
                let ifaddrs = &mut *ifaddrs_ptr;
                ifaddrs.ifa_next = 0; // 稍后填充
                ifaddrs.ifa_name = name_addr;
                ifaddrs.ifa_flags = get_interface_flags(iface.name());
                ifaddrs.ifa_addr = addr_addr;
                ifaddrs.ifa_netmask = netmask_addr;
                ifaddrs.ifa_ifu = broadcast_addr;
                ifaddrs.ifa_data = 0; // 不提供统计数据

                // 填充 sockaddr_in (addr)
                if let Some(ip_cidr) = ip_cidr_opt {
                    let addr_ptr = addr_addr as *mut SockAddrIn;
                    fill_sockaddr_from_ip(addr_ptr, ip_cidr.address());

                    // 填充 netmask
                    let netmask_ptr = netmask_addr as *mut SockAddrIn;
                    fill_sockaddr_from_netmask(netmask_ptr, ip_cidr.prefix_len());

                    // 填充 broadcast (如果是 IPv4 且不是回环)
                    if !iface.name().starts_with("lo") {
                        let broadcast_ptr = broadcast_addr as *mut SockAddrIn;
                        fill_sockaddr_broadcast(broadcast_ptr, ip_cidr);
                    }
                } else {
                    // 没有 IP 地址，清零
                    core::ptr::write_bytes(addr_addr as *mut u8, 0, sockaddr_size);
                    core::ptr::write_bytes(netmask_addr as *mut u8, 0, sockaddr_size);
                    core::ptr::write_bytes(broadcast_addr as *mut u8, 0, sockaddr_size);
                }

                // 填充接口名称
                let name_ptr = name_addr as *mut u8;
                core::ptr::copy_nonoverlapping(name_bytes.as_ptr(), name_ptr, name_bytes.len());
                *name_ptr.add(name_bytes.len()) = 0; // null terminator

                // 链接到前一个节点
                if prev_ifaddrs_addr != 0 {
                    let prev_ptr = prev_ifaddrs_addr as *mut IfAddrs;
                    (*prev_ptr).ifa_next = ifaddrs_addr;
                }

                prev_ifaddrs_addr = ifaddrs_addr;
            }
        }

        // 最后一个节点的 next 指针设为 null
        if prev_ifaddrs_addr != 0 {
            let prev_ptr = prev_ifaddrs_addr as *mut IfAddrs;
            (*prev_ptr).ifa_next = 0;
        }

        // 返回第一个 ifaddrs 的地址给用户
        *ifap = first_ifaddrs_addr as *mut u8;
    }

    0 // 成功
}

/// 获取接口标志
fn get_interface_flags(iface_name: &str) -> u32 {
    let mut flags = IFF_UP | IFF_RUNNING | IFF_MULTICAST;

    if iface_name.starts_with("lo") {
        flags |= IFF_LOOPBACK;
    } else {
        flags |= IFF_BROADCAST;
    }

    flags
}

/// 从 IP 地址填充 sockaddr_in
unsafe fn fill_sockaddr_from_ip(addr: *mut SockAddrIn, ip: smoltcp::wire::IpAddress) {
    use smoltcp::wire::IpAddress;

    let sockaddr = &mut *addr;
    sockaddr.sin_family = AF_INET;
    sockaddr.sin_port = 0;
    sockaddr.sin_zero = [0; 8];

    match ip {
        IpAddress::Ipv4(ipv4) => {
            sockaddr.sin_addr = ipv4.octets();
        }
        #[cfg(feature = "proto-ipv6")]
        IpAddress::Ipv6(_) => {
            // IPv6 需要不同的结构体，这里暂不支持
            sockaddr.sin_addr = [0; 4];
        }
        _ => {
            sockaddr.sin_addr = [0; 4];
        }
    }
}

/// 从前缀长度填充 netmask
unsafe fn fill_sockaddr_from_netmask(addr: *mut SockAddrIn, prefix_len: u8) {
    let sockaddr = &mut *addr;
    sockaddr.sin_family = AF_INET;
    sockaddr.sin_port = 0;
    sockaddr.sin_zero = [0; 8];

    // 计算 netmask (例如 /24 -> 255.255.255.0)
    let mask = if prefix_len == 0 {
        0u32
    } else if prefix_len >= 32 {
        0xFFFFFFFFu32
    } else {
        !((1u32 << (32 - prefix_len)) - 1)
    };

    sockaddr.sin_addr = [
        ((mask >> 24) & 0xFF) as u8,
        ((mask >> 16) & 0xFF) as u8,
        ((mask >> 8) & 0xFF) as u8,
        (mask & 0xFF) as u8,
    ];
}

/// 从 IP CIDR 填充广播地址
unsafe fn fill_sockaddr_broadcast(addr: *mut SockAddrIn, ip_cidr: &smoltcp::wire::IpCidr) {
    use smoltcp::wire::IpAddress;

    let sockaddr = &mut *addr;
    sockaddr.sin_family = AF_INET;
    sockaddr.sin_port = 0;
    sockaddr.sin_zero = [0; 8];

    match ip_cidr.address() {
        IpAddress::Ipv4(ipv4) => {
            let prefix_len = ip_cidr.prefix_len();
            let ip_u32 = u32::from_be_bytes(ipv4.octets());

            // 计算广播地址：IP | ~netmask
            let mask = if prefix_len >= 32 {
                0xFFFFFFFFu32
            } else {
                !((1u32 << (32 - prefix_len)) - 1)
            };

            let broadcast = ip_u32 | !mask;

            sockaddr.sin_addr = broadcast.to_be_bytes();
        }
        _ => {
            sockaddr.sin_addr = [255, 255, 255, 255];
        }
    }
}

// 释放获取网络接口列表分配的内存
pub fn freeifaddrs(ifa: *mut u8) -> isize {
    use crate::arch::trap::SumGuard;
    use crate::kernel::syscall::mm::munmap;
    use crate::uapi::errno::EINVAL;

    if ifa.is_null() {
        return 0; // NULL 指针，直接返回
    }

    let _guard = SumGuard::new();

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct IfAddrsAllocHeader {
        magic: u64,
        map_len: usize,
    }
    const IFADDRS_ALLOC_MAGIC: u64 = 0x434f4d49585f4946; // "COMIX_IF"
    const IFADDRS_HEADER_SIZE: usize = (core::mem::size_of::<IfAddrsAllocHeader>() + 7) & !7;

    let ifa_addr = ifa as usize;
    let header_addr = match ifa_addr.checked_sub(IFADDRS_HEADER_SIZE) {
        Some(v) => v,
        None => return -(EINVAL as isize),
    };

    unsafe {
        let header = (header_addr as *const IfAddrsAllocHeader).read();
        if header.magic != IFADDRS_ALLOC_MAGIC || header.map_len < IFADDRS_HEADER_SIZE {
            return -(EINVAL as isize);
        }

        let result = munmap(header_addr as *mut core::ffi::c_void, header.map_len);
        if result < 0 {
            return -(EINVAL as isize);
        }
    }

    0
}

// 设置网络接口配置
pub fn setsockopt(sockfd: i32, level: i32, optname: i32, optval: *const u8, optlen: u32) -> isize {
    use crate::arch::trap::SumGuard;
    use crate::kernel::current_cpu;
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

    let socket_file = match file
        .as_any()
        .downcast_ref::<crate::net::socket::SocketFile>()
    {
        Some(sf) => sf,
        None => return -(ENOTSOCK as isize),
    };

    let mut opts = socket_file.get_socket_options();

    {
        let _guard = SumGuard::new();
        unsafe {
            match level {
                SOL_SOCKET => match optname {
                    SO_REUSEADDR => set_sockopt_bool!(optval, optlen, opts.reuse_addr),
                    SO_REUSEPORT => set_sockopt_bool!(optval, optlen, opts.reuse_port),
                    SO_KEEPALIVE => set_sockopt_bool!(optval, optlen, opts.keepalive),
                    // Note: SO_SNDBUF/SO_RCVBUF are stored but not applied to smoltcp sockets
                    // smoltcp uses fixed-size buffers allocated at socket creation time
                    SO_SNDBUF => set_sockopt_int!(optval, optlen, opts.send_buffer_size),
                    SO_RCVBUF => set_sockopt_int!(optval, optlen, opts.recv_buffer_size),
                    SO_RCVTIMEO_OLD | SO_SNDTIMEO_OLD => { /* Ignore timeout options */ }
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
        }
    }

    socket_file.set_socket_options(opts);
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
    use crate::arch::trap::SumGuard;
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

    let socket_file = match file
        .as_any()
        .downcast_ref::<crate::net::socket::SocketFile>()
    {
        Some(sf) => sf,
        None => return -(ENOTSOCK as isize),
    };

    let opts = socket_file.get_socket_options();

    {
        let _guard = SumGuard::new();
        unsafe {
            let available_len = *optlen as usize;
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
                        core::ptr::copy_nonoverlapping(cc.as_ptr(), optval, n);
                        written_len = n;
                    }
                    TCP_INFO => {
                        // Best-effort placeholder. smoltcp doesn't currently expose full tcp_info metrics.
                        let info = TcpInfo::dummy_established();
                        let src = &info as *const TcpInfo as *const u8;
                        let n = core::cmp::min(available_len, core::mem::size_of::<TcpInfo>());
                        core::ptr::copy_nonoverlapping(src, optval, n);
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

            *optlen = written_len as u32;
        }
    }

    0
}

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
        let mut task_lock = task.lock();
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

    let endpoint = unsafe {
        let _guard = SumGuard::new();
        let ep = parse_sockaddr_in(dest_addr, addrlen);
        match ep {
            Ok(e) => e,
            Err(_) => return -22, // EINVAL
        }
    };

    let task = current_task();
    let tid = task.lock().tid as usize;

    let handle = match get_socket_handle(tid, sockfd as usize) {
        Some(h) => h,
        None => return -88, // ENOTSOCK
    };

    use crate::net::socket::socket_sendto;
    let result = {
        let _guard = SumGuard::new();
        let data = unsafe { core::slice::from_raw_parts(buf, len) };
        socket_sendto(handle, data, endpoint)
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
            let _guard = SumGuard::new();
            let data = unsafe { core::slice::from_raw_parts_mut(buf, len) };
            file.recvfrom(data)
        };

        match result {
            Ok((n, Some(addr_buf))) => {
                if !src_addr.is_null() && !addrlen.is_null() {
                    unsafe {
                        let _guard = SumGuard::new();
                        let len = (*addrlen as usize).min(addr_buf.len());
                        core::ptr::copy_nonoverlapping(addr_buf.as_ptr(), src_addr, len);
                        *addrlen = len as u32;
                    }
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
                    if let Some(socket_file) = file.as_any().downcast_ref::<SocketFile>() {
                        if !socket_file
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

    if how < 0 || how > 2 {
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

    if should_close_tcp {
        if let SocketHandle::Tcp(h) = handle {
            let mut sockets = SOCKET_SET.lock();
            let socket = sockets.get_mut::<tcp::Socket>(h);
            socket.close();
        }
    }

    0
}

// 获取套接字地址
pub fn getsockname(sockfd: i32, addr: *mut u8, addrlen: *mut u32) -> isize {
    let task = current_task();
    let tid = task.lock().tid as usize;

    let handle = match get_socket_handle(tid, sockfd as usize) {
        Some(h) => h,
        None => return -88, // ENOTSOCK
    };

    let sockets = SOCKET_SET.lock();
    let local_endpoint = match handle {
        SocketHandle::Tcp(h) => {
            let socket = sockets.get::<tcp::Socket>(h);
            socket.local_endpoint()
        }
        SocketHandle::Udp(h) => {
            let socket = sockets.get::<udp::Socket>(h);
            let listen_ep = socket.endpoint();
            Some(IpEndpoint::new(
                listen_ep
                    .addr
                    .unwrap_or(IpAddress::Ipv4(Ipv4Address::UNSPECIFIED)),
                listen_ep.port,
            ))
        }
    };

    drop(sockets);

    if let Some(ep) = local_endpoint {
        {
            let _guard = SumGuard::new();
            unsafe {
                if write_sockaddr_in(addr, addrlen, ep).is_err() {
                    return -22; // EINVAL
                }
            }
        }
        0
    } else {
        -22 // EINVAL
    }
}

// 获取对端套接字地址
pub fn getpeername(sockfd: i32, addr: *mut u8, addrlen: *mut u32) -> isize {
    let task = current_task();
    let tid = task.lock().tid as usize;

    let handle = match get_socket_handle(tid, sockfd as usize) {
        Some(h) => h,
        None => return -88, // ENOTSOCK
    };

    let sockets = SOCKET_SET.lock();
    let remote_endpoint = match handle {
        SocketHandle::Tcp(h) => {
            let socket = sockets.get::<tcp::Socket>(h);
            socket.remote_endpoint()
        }
        SocketHandle::Udp(_) => {
            // UDP doesn't have a peer, use stored endpoint
            drop(sockets);
            let file = match task.lock().fd_table.get(sockfd as usize) {
                Ok(f) => f,
                Err(_) => return -9, // EBADF
            };
            use crate::net::socket::get_socket_remote_endpoint;
            get_socket_remote_endpoint(&file)
        }
    };

    if let Some(ep) = remote_endpoint {
        {
            let _guard = SumGuard::new();
            unsafe {
                if write_sockaddr_in(addr, addrlen, ep).is_err() {
                    return -22; // EINVAL
                }
            }
        }
        0
    } else {
        -107 // ENOTCONN
    }
}
