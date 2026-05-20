//! 网络相关的系统调用实现

use alloc::string::{String, ToString};
use core::ffi::c_char;
use core::sync::atomic::{AtomicU16, Ordering};

use crate::arch::Arch;

use crate::util::user_buffer::{read_from_user, write_to_user};

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
            let val: i32 = read_from_user($optval as *const i32);
            $field = val != 0;
        }
    };
}

macro_rules! set_sockopt_int {
    ($optval:expr, $optlen:expr, $field:expr) => {
        if $optlen >= 4 {
            let val: i32 = read_from_user($optval as *const i32);
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
            write_to_user($optval as *mut i32, if $field { 1 } else { 0 });
            $written = 4;
        }
    };
}

macro_rules! get_sockopt_int {
    ($optval:expr, $avail:expr, $field:expr, $written:expr) => {
        if $avail >= 4 {
            write_to_user($optval as *mut i32, $field as i32);
            $written = 4;
        }
    };
}

use crate::vfs::File;
use crate::{
    kernel::current_task,
    net::{
        interface::NETWORK_INTERFACE_MANAGER,
        socket::{
            SocketFile, SocketHandle, create_tcp_socket, create_udp_socket, get_socket_handle,
            parse_sockaddr_in, register_socket_fd, unregister_socket_fd, write_sockaddr_in,
        },
        stack::{TcpConnectionState, TcpListenState, network_stack},
    },
    pr_debug, pr_info, println,
    uapi::{
        fcntl::{FdFlags, OpenFlags},
        socket::{SOCK_CLOEXEC, SOCK_DGRAM, SOCK_NONBLOCK, SOCK_STREAM, SOCK_TYPE_MASK},
    },
};
use alloc::sync::Arc;
use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address};

/// 安全地从用户空间拷贝C字符串
fn copy_c_str_from_user(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let mut buf = [0u8; 256];
    match unsafe {
        crate::arch::ArchImpl::copy_strn_from_user(
            crate::arch::address::UA::from_usize(ptr as usize),
            buf.as_mut_ptr(),
            buf.len(),
        )
    } {
        Ok(len) if len > 0 => {
            let s = core::str::from_utf8(&buf[..len]).ok()?;
            Some(s.to_string())
        }
        _ => None,
    }
}

mod addr_ops;
mod connection_ops;
mod ifaddrs_ops;
mod socket_ops;
mod sockopt_ops;

pub use addr_ops::*;
pub use connection_ops::*;
pub use ifaddrs_ops::*;
pub use socket_ops::*;
pub use sockopt_ops::*;
