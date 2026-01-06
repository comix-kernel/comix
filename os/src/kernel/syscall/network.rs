//! 网络相关的系统调用实现

use core::ffi::{CStr, c_char};

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
            $field = val as usize;
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

use crate::{
    arch::trap::SumGuard,
    kernel::{current_cpu, current_task},
    net::{
        config::NetworkConfigManager,
        interface::NETWORK_INTERFACE_MANAGER,
        socket::{
            SOCKET_SET, SocketFile, SocketHandle, create_tcp_socket, create_udp_socket,
            get_socket_handle, parse_sockaddr_in, register_socket_fd, unregister_socket_fd,
            write_sockaddr_in,
        },
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
                crate::pr_debug!("Network config error: {:?}", e);
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

    let handle = match socket_type {
        1 => match create_tcp_socket() {
            Ok(h) => h,
            Err(_) => return -12, // ENOMEM
        },
        2 => match create_udp_socket() {
            Ok(h) => h,
            Err(_) => return -12, // ENOMEM
        },
        _ => return -94, // ESOCKTNOSUPPORT
    };

    let socket_file = Arc::new(SocketFile::new(handle));
    let task = current_task();

    let mut task_lock = task.lock();
    match task_lock.fd_table.alloc(socket_file) {
        Ok(fd) => {
            register_socket_fd(task_lock.tid as usize, fd, handle);
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
            let mut sockets = SOCKET_SET.lock();
            let socket = sockets.get_mut::<udp::Socket>(h);
            if socket.bind(endpoint).is_err() {
                return -98; // EADDRINUSE
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

            let mut sockets = SOCKET_SET.lock();
            let socket = sockets.get_mut::<tcp::Socket>(h);
            if socket.listen(endpoint).is_err() {
                return -98; // EADDRINUSE
            }
            0
        }
        SocketHandle::Udp(_) => -95, // EOPNOTSUPP - UDP doesn't support listen
    }
}

/// 接受连接
pub fn accept(sockfd: i32, addr: *mut u8, addrlen: *mut u32) -> isize {
    let task = current_task();
    let tid = task.lock().tid;

    let listen_handle = match get_socket_handle(tid as usize, sockfd as usize) {
        Some(SocketHandle::Tcp(h)) => h,
        Some(SocketHandle::Udp(_)) => return -95, // EOPNOTSUPP
        None => return -88,                       // ENOTSOCK
    };

    let mut sockets = SOCKET_SET.lock();
    let listen_socket = sockets.get_mut::<tcp::Socket>(listen_handle);

    if !listen_socket.is_listening() {
        return -22; // EINVAL - not in listening state
    }

    if !listen_socket.is_active() {
        return -11; // EAGAIN - no pending connection
    }

    // Get remote endpoint and local endpoint
    let remote_endpoint = match listen_socket.remote_endpoint() {
        Some(ep) => ep,
        None => return -11, // EAGAIN
    };
    let local_endpoint = listen_socket.local_endpoint().unwrap();

    // Create new listening socket to replace the old one
    let new_listen_handle = match create_tcp_socket() {
        Ok(SocketHandle::Tcp(h)) => h,
        _ => return -12, // ENOMEM
    };

    // Set new socket to listen on the same address
    let new_listen_socket = sockets.get_mut::<tcp::Socket>(new_listen_handle);
    if new_listen_socket.listen(local_endpoint).is_err() {
        sockets.remove(new_listen_handle);
        return -12; // ENOMEM or other error
    }

    // The old listen_handle is now the established connection
    // Update the mapping to point to the new listening socket
    use crate::net::socket::{update_socket_file_handle, update_socket_handle};
    update_socket_handle(
        tid as usize,
        sockfd as usize,
        SocketHandle::Tcp(new_listen_handle),
    );

    // Also update the SocketFile's internal handle
    let file = match task.lock().fd_table.get(sockfd as usize) {
        Ok(f) => f,
        Err(_) => return -9, // EBADF
    };
    update_socket_file_handle(&file, SocketHandle::Tcp(new_listen_handle)).unwrap();

    drop(sockets);

    // Write address info if requested
    if !addr.is_null() && !addrlen.is_null() {
        let _guard = SumGuard::new();
        unsafe {
            let _ = write_sockaddr_in(addr, addrlen, remote_endpoint);
        }
    }

    // Return the established connection as a new fd
    let conn_handle = SocketHandle::Tcp(listen_handle);
    let socket_file = Arc::new(SocketFile::new(conn_handle));
    match task.lock().fd_table.alloc(socket_file) {
        Ok(fd) => {
            register_socket_fd(tid as usize, fd, conn_handle);
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
            Ok(e) => e,
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
            if socket.is_open() {
                return -106; // EISCONN
            }
            let local_endpoint = socket.local_endpoint().unwrap_or(IpEndpoint::new(
                IpAddress::Ipv4(Ipv4Address::UNSPECIFIED),
                0,
            ));
            drop(sockets);

            use crate::net::socket::tcp_connect;
            if let Err(_) = tcp_connect(h, endpoint, local_endpoint) {
                return -22; // EINVAL or connection error
            }

            if is_nonblock {
                return -115; // EINPROGRESS
            }
        }
        SocketHandle::Udp(_) => {}
    }

    0
}

/// 发送数据
pub fn send(sockfd: i32, buf: *const u8, len: usize, _flags: i32) -> isize {
    let data = unsafe {
        let _guard = SumGuard::new();
        let slice = core::slice::from_raw_parts(buf, len);
        slice
    };

    let task = current_task();
    let file = match task.lock().fd_table.get(sockfd as usize) {
        Ok(f) => f,
        Err(_) => return -9,
    };

    match file.write(data) {
        Ok(n) => n as isize,
        Err(_) => -11, // EAGAIN
    }
}

/// 接收数据
pub fn recv(sockfd: i32, buf: *mut u8, len: usize, _flags: i32) -> isize {
    let task = current_task();
    let file = match task.lock().fd_table.get(sockfd as usize) {
        Ok(f) => f,
        Err(_) => return -9,
    };

    let data = unsafe {
        let _guard = SumGuard::new();
        let slice = core::slice::from_raw_parts_mut(buf, len);
        slice
    };

    match file.read(data) {
        Ok(n) => n as isize,
        Err(_) => -11, // EAGAIN
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
fn get_interface_stats(ifname: *const c_char, stats: *mut u8, size: usize) -> isize {
    // TODO: 实现统计信息获取
    0 // 返回0表示成功
}

pub fn init_network_syscalls() {
    crate::pr_debug!("Network syscalls initialized");
}

/// 获取网络接口地址列表 (Linux标准系统调用)
pub fn getifaddrs(ifap: *mut *mut u8) -> isize {
    unsafe {
        let _guard = SumGuard::new();

        // 获取所有网络接口
        let interfaces = NETWORK_INTERFACE_MANAGER.lock().get_interfaces().to_vec();

        if interfaces.is_empty() {
            return -1; // ENOENT
        }

        // 简化实现：返回成功，但不填充实际数据
        // 在实际实现中，需要分配内存并填充ifaddrs结构
        0 // 成功
    }
}

// 释放获取网络接口列表分配的内存
pub fn freeifaddrs(ifa: *mut u8) -> isize {
    unsafe {
        let _guard = SumGuard::new();

        // 简化实现：不执行任何操作
        // 在实际实现中，需要释放getifaddrs分配的内存

        0
    }
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
                    SO_SNDBUF => set_sockopt_int!(optval, optlen, opts.send_buffer_size),
                    SO_RCVBUF => set_sockopt_int!(optval, optlen, opts.recv_buffer_size),
                    _ => return -(ENOPROTOOPT as isize),
                },
                IPPROTO_TCP => match optname {
                    TCP_NODELAY => set_sockopt_bool!(optval, optlen, opts.tcp_nodelay),
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
    use crate::kernel::current_cpu;
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
    unsafe {
        let _guard = SumGuard::new();

        // TODO: 实现接受连接逻辑
        // 检查套接字是否有效
        if sockfd < 0 {
            return -1; // EBADF
        }

        // 暂时返回一个虚拟的文件描述符
        4
    }
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

    let data = unsafe {
        let _guard = SumGuard::new();
        let slice = core::slice::from_raw_parts(buf, len);
        slice
    };

    let task = current_task();
    let tid = task.lock().tid as usize;

    let handle = match get_socket_handle(tid, sockfd as usize) {
        Some(h) => h,
        None => return -88, // ENOTSOCK
    };

    use crate::net::socket::socket_sendto;
    match socket_sendto(handle, data, endpoint) {
        Ok(n) => n as isize,
        Err(e) => e.to_errno(),
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
    let task = current_task();
    let file = match task.lock().fd_table.get(sockfd as usize) {
        Ok(f) => f,
        Err(_) => return -9, // EBADF
    };

    let data = unsafe {
        let _guard = SumGuard::new();
        let slice = core::slice::from_raw_parts_mut(buf, len);
        slice
    };

    match file.recvfrom(data) {
        Ok((n, Some(addr_buf))) => {
            if !src_addr.is_null() && !addrlen.is_null() {
                unsafe {
                    let _guard = SumGuard::new();
                    let len = (*addrlen as usize).min(addr_buf.len());
                    core::ptr::copy_nonoverlapping(addr_buf.as_ptr(), src_addr, len);
                    *addrlen = len as u32;
                }
            }
            n as isize
        }
        Ok((n, None)) => n as isize,
        Err(_) => -11, // EAGAIN
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
            listen_ep
                .addr
                .map(|addr| IpEndpoint::new(addr, listen_ep.port))
        }
    };

    drop(sockets);

    if let Some(ep) = local_endpoint {
        {
            let _guard = SumGuard::new();
            unsafe {
                let _ = write_sockaddr_in(addr, addrlen, ep);
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
                let _ = write_sockaddr_in(addr, addrlen, ep);
            }
        }
        0
    } else {
        -107 // ENOTCONN
    }
}
