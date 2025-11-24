//! 网络系统调用实现
use core::ffi::{CStr, c_char};

use riscv::register::sstatus;

use crate::{
    device::net::{config::NetworkConfigManager, interface::NETWORK_INTERFACE_MANAGER},
    println,
};

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
pub fn socket(domain: i32, socket_type: i32, protocol: i32) -> isize {
    // 目前只支持IPv4和TCP/UDP
    if domain != 2 || (socket_type != 1 && socket_type != 2) {
        return -1; // EAFNOSUPPORT 或 ESOCKTNOSUPPORT
    }

    // TODO: 实现套接字创建
    // 暂时返回一个虚拟的文件描述符
    3
}

/// 绑定套接字
pub fn bind(sockfd: i32, addr: *const u8, addrlen: u32) -> isize {
    // TODO: 实现绑定逻辑
    0
}

/// 监听连接
pub fn listen(sockfd: i32, backlog: i32) -> isize {
    // TODO: 实现监听逻辑
    0
}

/// 接受连接
pub fn accept(sockfd: i32, addr: *mut u8, addrlen: *mut u32) -> isize {
    // TODO: 实现接受连接逻辑
    // 暂时返回一个虚拟的文件描述符
    4
}

/// 连接到远程地址
pub fn connect(sockfd: i32, addr: *const u8, addrlen: u32) -> isize {
    // TODO: 实现连接逻辑
    0
}

/// 发送数据
pub fn send(sockfd: i32, buf: *const u8, len: usize, flags: i32) -> isize {
    // TODO: 实现发送逻辑
    len as isize
}

/// 接收数据
pub fn recv(sockfd: i32, buf: *mut u8, len: usize, flags: i32) -> isize {
    // TODO: 实现接收逻辑
    // 暂时返回0，表示没有数据可读
    0
}

/// 关闭套接字
pub fn close(sockfd: i32) -> isize {
    // TODO: 实现关闭逻辑
    0
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
    flags: i32,
    dest_addr: *const u8,
    addrlen: u32,
) -> isize {
    unsafe {
        sstatus::set_sum();

        // TODO: 实现发送逻辑
        // 检查套接字、缓冲区和地址是否有效
        if sockfd < 0 || buf.is_null() || dest_addr.is_null() {
            sstatus::clear_sum();
            return -1; // EBADF 或 EFAULT
        }

        sstatus::clear_sum();
        len as isize // 假设所有数据都已发送
    }
}

// Linux 标准: ssize_t recvfrom(int sockfd, void *buf, size_t len, int flags, struct sockaddr *src_addr, socklen_t *addrlen);
pub fn recvfrom(
    sockfd: i32,
    buf: *mut u8,
    len: usize,
    flags: i32,
    src_addr: *mut u8,
    addrlen: *mut u32,
) -> isize {
    unsafe {
        sstatus::set_sum();

        // TODO: 实现接收逻辑
        // 检查套接字和缓冲区是否有效
        if sockfd < 0 || buf.is_null() {
            sstatus::clear_sum();
            return -1; // EBADF 或 EFAULT
        }

        // 暂时返回0，表示没有数据可读
        sstatus::clear_sum();
        0
    }
}

// 关闭套接字
pub fn shutdown(sockfd: i32, how: i32) -> isize {
    unsafe {
        sstatus::set_sum();

        // TODO: 实现关闭套接字逻辑
        // 检查套接字是否有效
        if sockfd < 0 {
            sstatus::clear_sum();
            return -1; // EBADF
        }

        // 检查how参数是否有效
        if how < 0 || how > 2 {
            sstatus::clear_sum();
            return -1; // EINVAL
        }

        sstatus::clear_sum();
        0 // 成功
    }
}

// 获取套接字地址
pub fn getsockname(sockfd: i32, addr: *mut u8, addrlen: *mut u32) -> isize {
    unsafe {
        sstatus::set_sum();

        // TODO: 实现获取套接字名称逻辑
        // 检查套接字是否有效
        if sockfd < 0 {
            sstatus::clear_sum();
            return -1; // EBADF
        }

        // 检查addr和addrlen是否有效
        if addr.is_null() || addrlen.is_null() {
            sstatus::clear_sum();
            return -1; // EFAULT
        }

        sstatus::clear_sum();
        0 // 成功
    }
}

// 获取对端套接字地址
pub fn getpeername(sockfd: i32, addr: *mut u8, addrlen: *mut u32) -> isize {
    unsafe {
        sstatus::set_sum();

        // TODO: 实现获取对端套接字名称逻辑
        // 检查套接字是否有效
        if sockfd < 0 {
            sstatus::clear_sum();
            return -1; // EBADF
        }

        // 检查addr和addrlen是否有效
        if addr.is_null() || addrlen.is_null() {
            sstatus::clear_sum();
            return -1; // EFAULT
        }

        sstatus::clear_sum();
        0 // 成功
    }
}

// 获取网络接口统计信息
// Linux 标准: int ioctl(int sockfd, unsigned long request, ...);
pub fn ioctl(sockfd: i32, request: u32, arg: *mut u8) -> isize {
    unsafe {
        sstatus::set_sum();

        // TODO: 实现ioctl逻辑
        // 检查套接字是否有效
        if sockfd < 0 {
            sstatus::clear_sum();
            return -1; // EBADF
        }

        // 暂时返回成功
        sstatus::clear_sum();
        0
    }
}
