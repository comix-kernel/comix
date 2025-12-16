//! 网络相关的系统调用实现

use core::ffi::{CStr, c_char};

use riscv::register::sstatus;

use crate::{
    kernel::current_cpu,
    net::{
        config::NetworkConfigManager,
        interface::NETWORK_INTERFACE_MANAGER,
        socket::{
            SOCKET_SET, SocketFile, SocketHandle, create_tcp_socket, create_udp_socket,
            get_socket_handle, parse_sockaddr_in, register_socket_fd, unregister_socket_fd,
            write_sockaddr_in,
        },
    },
    println,
};
use alloc::sync::Arc;
use smoltcp::socket::{tcp, udp};

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
        sstatus::set_sum();

        let ifname_str = match get_c_str_safe(ifname) {
            Some(s) => s,
            None => {
                sstatus::clear_sum();
                return -1;
            }
        };

        let ip_str = match get_c_str_safe(ip) {
            Some(s) => s,
            None => {
                sstatus::clear_sum();
                return -2;
            }
        };

        let gateway_str = match get_c_str_safe(gateway) {
            Some(s) => s,
            None => {
                sstatus::clear_sum();
                return -3;
            }
        };

        let mask_str = match get_c_str_safe(mask) {
            Some(s) => s,
            None => {
                sstatus::clear_sum();
                return -4;
            }
        };

        sstatus::clear_sum();

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
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();

    match task.lock().fd_table.alloc(socket_file) {
        Ok(fd) => {
            register_socket_fd(task.lock().tid, fd, handle);
            fd as isize
        }
        Err(_) => -24, // EMFILE
    }
}

/// 绑定套接字
pub fn bind(sockfd: i32, addr: *const u8, addrlen: u32) -> isize {
    let endpoint = unsafe {
        sstatus::set_sum();
        let ep = parse_sockaddr_in(addr, addrlen);
        sstatus::clear_sum();
        match ep {
            Ok(e) => e,
            Err(_) => return -22, // EINVAL
        }
    };

    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    let tid = task.lock().tid;

    let handle = match get_socket_handle(tid, sockfd as usize) {
        Some(h) => h,
        None => return -88, // ENOTSOCK
    };

    let mut sockets = SOCKET_SET.lock();
    match handle {
        SocketHandle::Tcp(h) => {
            let socket = sockets.get_mut::<tcp::Socket>(h);
            if socket.listen(endpoint).is_err() {
                return -98; // EADDRINUSE
            }
        }
        SocketHandle::Udp(h) => {
            let socket = sockets.get_mut::<udp::Socket>(h);
            if socket.bind(endpoint).is_err() {
                return -98; // EADDRINUSE
            }
        }
    }

    0
}

/// 监听连接
pub fn listen(sockfd: i32, _backlog: i32) -> isize {
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    let tid = task.lock().tid;

    let handle = match get_socket_handle(tid, sockfd as usize) {
        Some(h) => h,
        None => return -88, // ENOTSOCK
    };

    match handle {
        SocketHandle::Tcp(_) => 0,
        SocketHandle::Udp(_) => -95, // EOPNOTSUPP - UDP doesn't support listen
    }
}

/// 接受连接
pub fn accept(sockfd: i32, addr: *mut u8, addrlen: *mut u32) -> isize {
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    let tid = task.lock().tid;

    let listen_handle = match get_socket_handle(tid, sockfd as usize) {
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

    // Get remote endpoint before accepting
    let remote_endpoint = match listen_socket.remote_endpoint() {
        Some(ep) => ep,
        None => return -11, // EAGAIN
    };

    drop(sockets);

    // Create new socket for the accepted connection
    let new_handle = match create_tcp_socket() {
        Ok(h) => h,
        Err(_) => return -12, // ENOMEM
    };

    // Write address info if requested
    if !addr.is_null() && !addrlen.is_null() {
        unsafe {
            sstatus::set_sum();
            let _ = write_sockaddr_in(addr, addrlen, remote_endpoint);
            sstatus::clear_sum();
        }
    }

    let socket_file = Arc::new(SocketFile::new(new_handle));
    match task.lock().fd_table.alloc(socket_file) {
        Ok(fd) => {
            register_socket_fd(tid, fd, new_handle);
            fd as isize
        }
        Err(_) => -24, // EMFILE
    }
}

/// 连接到远程地址
pub fn connect(sockfd: i32, addr: *const u8, addrlen: u32) -> isize {
    let _endpoint = unsafe {
        sstatus::set_sum();
        let ep = parse_sockaddr_in(addr, addrlen);
        sstatus::clear_sum();
        match ep {
            Ok(e) => e,
            Err(_) => return -22,
        }
    };

    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    match task.lock().fd_table.get(sockfd as usize) {
        Ok(_) => 0,
        Err(_) => -9,
    }
}

/// 发送数据
pub fn send(sockfd: i32, buf: *const u8, len: usize, _flags: i32) -> isize {
    let data = unsafe {
        sstatus::set_sum();
        let slice = core::slice::from_raw_parts(buf, len);
        sstatus::clear_sum();
        slice
    };

    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
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
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    let file = match task.lock().fd_table.get(sockfd as usize) {
        Ok(f) => f,
        Err(_) => return -9,
    };

    let data = unsafe {
        sstatus::set_sum();
        let slice = core::slice::from_raw_parts_mut(buf, len);
        sstatus::clear_sum();
        slice
    };

    match file.read(data) {
        Ok(n) => n as isize,
        Err(_) => -11, // EAGAIN
    }
}

/// 关闭套接字
pub fn close_sock(sockfd: i32) -> isize {
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    let tid = task.lock().tid;

    unregister_socket_fd(tid, sockfd as usize);

    match task.lock().fd_table.close(sockfd as usize) {
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
    println!("Network syscalls initialized");
}

/// 获取网络接口地址列表 (Linux标准系统调用)
pub fn getifaddrs(ifap: *mut *mut u8) -> isize {
    unsafe {
        sstatus::set_sum();

        // 获取所有网络接口
        let interfaces = NETWORK_INTERFACE_MANAGER.lock().get_interfaces().to_vec();

        if interfaces.is_empty() {
            sstatus::clear_sum();
            return -1; // ENOENT
        }

        // 简化实现：返回成功，但不填充实际数据
        // 在实际实现中，需要分配内存并填充ifaddrs结构
        sstatus::clear_sum();
        0 // 成功
    }
}

// 释放获取网络接口列表分配的内存
pub fn freeifaddrs(ifa: *mut u8) -> isize {
    unsafe {
        sstatus::set_sum();

        // 简化实现：不执行任何操作
        // 在实际实现中，需要释放getifaddrs分配的内存

        sstatus::clear_sum();
        0
    }
}

// 设置网络接口配置
pub fn setsockopt(sockfd: i32, level: i32, optname: i32, optval: *const u8, optlen: u32) -> isize {
    unsafe {
        sstatus::set_sum();

        // TODO: 实现设置套接字选项逻辑
        // 检查套接字是否有效
        if sockfd < 0 {
            sstatus::clear_sum();
            return -1; // EBADF
        }

        // 检查optval是否有效
        if optval.is_null() {
            sstatus::clear_sum();
            return -1; // EFAULT
        }

        sstatus::clear_sum();
        0 // 成功
    }
}

// 获取网络接口配置
pub fn getsockopt(
    sockfd: i32,
    level: i32,
    optname: i32,
    optval: *mut u8,
    optlen: *mut u32,
) -> isize {
    unsafe {
        sstatus::set_sum();

        // TODO: 实现获取套接字选项逻辑
        // 检查套接字是否有效
        if sockfd < 0 {
            sstatus::clear_sum();
            return -1; // EBADF
        }

        // 检查optval和optlen是否有效
        if optval.is_null() || optlen.is_null() {
            sstatus::clear_sum();
            return -1; // EFAULT
        }

        sstatus::clear_sum();
        0 // 成功
    }
}

// 接受连接（非阻塞）
pub fn accept4(sockfd: i32, addr: *mut u8, addrlen: *mut u32, flags: i32) -> isize {
    unsafe {
        sstatus::set_sum();

        // TODO: 实现接受连接逻辑
        // 检查套接字是否有效
        if sockfd < 0 {
            sstatus::clear_sum();
            return -1; // EBADF
        }

        // 暂时返回一个虚拟的文件描述符
        sstatus::clear_sum();
        4
    }
}

// 发送数据到指定地址
pub fn sendto(
    sockfd: i32,
    buf: *const u8,
    len: usize,
    _flags: i32,
    _dest_addr: *const u8,
    _addrlen: u32,
) -> isize {
    send(sockfd, buf, len, 0)
}

// Linux 标准: ssize_t recvfrom(int sockfd, void *buf, size_t len, int flags, struct sockaddr *src_addr, socklen_t *addrlen);
pub fn recvfrom(
    sockfd: i32,
    buf: *mut u8,
    len: usize,
    _flags: i32,
    _src_addr: *mut u8,
    _addrlen: *mut u32,
) -> isize {
    recv(sockfd, buf, len, 0)
}

// 关闭套接字
pub fn shutdown(sockfd: i32, how: i32) -> isize {
    if how < 0 || how > 2 {
        return -22;
    }
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    match task.lock().fd_table.get(sockfd as usize) {
        Ok(_) => 0,
        Err(_) => -9,
    }
}

// 获取套接字地址
pub fn getsockname(sockfd: i32, _addr: *mut u8, _addrlen: *mut u32) -> isize {
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    match task.lock().fd_table.get(sockfd as usize) {
        Ok(_) => 0,
        Err(_) => -9,
    }
}

// 获取对端套接字地址
pub fn getpeername(sockfd: i32, _addr: *mut u8, _addrlen: *mut u32) -> isize {
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    match task.lock().fd_table.get(sockfd as usize) {
        Ok(_) => 0,
        Err(_) => -9,
    }
}
