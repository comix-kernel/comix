use super::*;

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

    // 解析接口名称
    let if_name_str = match copy_c_str_from_user(ifname) {
        Some(s) => s,
        None => return -(EINVAL as isize),
    };

    // 查找网络接口
    let iface_manager = NETWORK_INTERFACE_MANAGER.lock();
    let interface = match iface_manager.find_interface_by_name(&if_name_str) {
        Some(iface) => iface,
        None => return -(ENODEV as isize),
    };

    // 获取设备统计信息
    let _device = interface.device();

    // 清零整个统计结构 (struct rtnl_link_stats64)
    let zero_buf = alloc::vec![0u8; size];
    unsafe {
        crate::arch::ArchImpl::copy_to_user(
            zero_buf.as_ptr(),
            crate::arch::address::UA::from_usize(stats as usize),
            size,
        )
        .ok();
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
    use crate::uapi::errno::{EFAULT, ENOMEM};

    if ifap.is_null() {
        return -(EFAULT as isize);
    }

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
        write_to_user(ifap, core::ptr::null_mut::<u8>());
        return 0;
    }

    // 计算需要的总内存大小
    let mut total_size = 0usize;
    let ifaddrs_size = core::mem::size_of::<IfAddrs>();
    let sockaddr_size = core::mem::size_of::<SockAddrIn>();

    for iface in interfaces.iter() {
        total_size += ifaddrs_size;
        total_size += sockaddr_size * 3;
        total_size += iface.name().len() + 1;
        let ip_count = iface.ip_addresses().len().max(1);
        total_size += (ifaddrs_size + sockaddr_size * 3) * (ip_count.saturating_sub(1));
    }
    total_size = (total_size + 7) & !7;

    // 使用 mmap 在用户空间分配内存
    let (user_mem_start, map_len) = {
        use crate::config::PAGE_SIZE;
        use crate::kernel::syscall::mm::mmap;
        use crate::uapi::mm::{MapFlags, ProtFlags};

        let map_len = {
            let raw = total_size + IFADDRS_HEADER_SIZE;
            (raw + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
        };

        let addr = mmap(
            core::ptr::null_mut(),
            map_len,
            (ProtFlags::READ | ProtFlags::WRITE).bits(),
            (MapFlags::ANONYMOUS | MapFlags::PRIVATE).bits(),
            -1,
            0,
        );

        if addr <= 0 {
            return -(ENOMEM as isize);
        }

        (addr as usize + IFADDRS_HEADER_SIZE, map_len)
    };

    // 写入 header
    write_to_user(
        (user_mem_start - IFADDRS_HEADER_SIZE) as *mut IfAddrsAllocHeader,
        IfAddrsAllocHeader {
            magic: IFADDRS_ALLOC_MAGIC,
            map_len,
        },
    );

    // 在 kernel buffer 中构建整个数据结构（使用绝对用户态地址）
    let mut kernel_buf = alloc::vec![0u8; total_size];
    let mut current_offset = 0usize;
    let mut first_ifaddrs_addr = 0usize;
    let mut prev_ifaddrs_offset = 0usize;

    for iface in interfaces.iter() {
        let ip_addrs = iface.ip_addresses();
        let ip_list = if ip_addrs.is_empty() {
            alloc::vec![None]
        } else {
            ip_addrs
                .iter()
                .map(|ip| Some(*ip))
                .collect::<alloc::vec::Vec<_>>()
        };

        for ip_cidr_opt in ip_list.iter() {
            let ifaddrs_off = current_offset;
            let ifaddrs_ua = user_mem_start + ifaddrs_off;
            if first_ifaddrs_addr == 0 {
                first_ifaddrs_addr = ifaddrs_ua;
            }
            current_offset += ifaddrs_size;

            let addr_off = current_offset;
            let addr_ua = user_mem_start + addr_off;
            current_offset += sockaddr_size;

            let netmask_off = current_offset;
            let netmask_ua = user_mem_start + netmask_off;
            current_offset += sockaddr_size;

            let broadcast_off = current_offset;
            let broadcast_ua = user_mem_start + broadcast_off;
            current_offset += sockaddr_size;

            let name_off = current_offset;
            let name_ua = user_mem_start + name_off;
            let name_bytes = iface.name().as_bytes();
            current_offset += name_bytes.len() + 1;
            current_offset = (current_offset + 7) & !7;

            // 填充 IfAddrs 结构体到 kernel buffer
            let ifa_slice = &mut kernel_buf[ifaddrs_off..ifaddrs_off + ifaddrs_size];
            // ifa_next: usize (offset 0)
            ifa_slice[0..8].copy_from_slice(&0usize.to_ne_bytes());
            // ifa_name: usize (offset 8)
            ifa_slice[8..16].copy_from_slice(&name_ua.to_ne_bytes());
            // ifa_flags: u32 (offset 16)
            ifa_slice[16..20].copy_from_slice(&get_interface_flags(iface.name()).to_ne_bytes());
            // ifa_addr: usize (offset 24)
            ifa_slice[24..32].copy_from_slice(&addr_ua.to_ne_bytes());
            // ifa_netmask: usize (offset 32)
            ifa_slice[32..40].copy_from_slice(&netmask_ua.to_ne_bytes());
            // ifa_ifu: usize (offset 40)
            ifa_slice[40..48].copy_from_slice(&broadcast_ua.to_ne_bytes());
            // ifa_data: usize (offset 48)
            ifa_slice[48..56].fill(0);

            // 填充 sockaddr (使用 split_at_mut 避免多重借用冲突，布局保证 addr/netmask/broadcast 连续)
            let (_pre, rest) = kernel_buf.split_at_mut(addr_off);
            let (addr_slice, rest) = rest.split_at_mut(sockaddr_size);
            let (netmask_slice, rest) = rest.split_at_mut(sockaddr_size);
            let (broadcast_slice, _rest) = rest.split_at_mut(sockaddr_size);

            if let Some(ip_cidr) = ip_cidr_opt {
                fill_sockaddr_from_ip(addr_slice, ip_cidr.address());
                fill_sockaddr_from_netmask(netmask_slice, ip_cidr.prefix_len());
                if !iface.name().starts_with("lo") {
                    fill_sockaddr_broadcast(broadcast_slice, ip_cidr);
                } else {
                    broadcast_slice.fill(0);
                }
            } else {
                addr_slice.fill(0);
                netmask_slice.fill(0);
                broadcast_slice.fill(0);
            }

            // 填充接口名称
            let name_slice = &mut kernel_buf[name_off..name_off + name_bytes.len() + 1];
            name_slice[..name_bytes.len()].copy_from_slice(name_bytes);
            name_slice[name_bytes.len()] = 0;

            // 链接到前一个节点
            if prev_ifaddrs_offset != 0 {
                let prev_ifa_slice = &mut kernel_buf[prev_ifaddrs_offset..prev_ifaddrs_offset + 8];
                prev_ifa_slice.copy_from_slice(&ifaddrs_ua.to_ne_bytes());
            }

            prev_ifaddrs_offset = ifaddrs_off;
        }
    }

    // 最后一个节点的 next 指针已经填充为 0（初始化时）

    // 复制整个结构到用户空间
    unsafe {
        crate::arch::ArchImpl::copy_to_user(
            kernel_buf.as_ptr(),
            crate::arch::address::UA::from_usize(user_mem_start),
            total_size,
        )
        .ok();
    }

    // 返回第一个 ifaddrs 的地址给用户
    write_to_user(ifap, first_ifaddrs_addr as *mut u8);

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

/// 从 IP 地址填充 sockaddr_in 到字节缓冲区
fn fill_sockaddr_from_ip(buf: &mut [u8], ip: smoltcp::wire::IpAddress) {
    use smoltcp::wire::IpAddress;
    buf[0..2].copy_from_slice(&AF_INET.to_ne_bytes());
    buf[2..4].copy_from_slice(&0u16.to_ne_bytes()); // port = 0
    buf[8..16].fill(0); // sin_zero

    match ip {
        IpAddress::Ipv4(ipv4) => {
            buf[4..8].copy_from_slice(&ipv4.octets());
        }
        _ => {
            buf[4..8].fill(0);
        }
    }
}

/// 从前缀长度填充 netmask 到字节缓冲区
fn fill_sockaddr_from_netmask(buf: &mut [u8], prefix_len: u8) {
    buf[0..2].copy_from_slice(&AF_INET.to_ne_bytes());
    buf[2..4].copy_from_slice(&0u16.to_ne_bytes()); // port = 0
    buf[8..16].fill(0); // sin_zero

    // 计算 netmask (例如 /24 -> 255.255.255.0)
    let mask = if prefix_len == 0 {
        0u32
    } else if prefix_len >= 32 {
        0xFFFFFFFFu32
    } else {
        !((1u32 << (32 - prefix_len)) - 1)
    };

    buf[4..8].copy_from_slice(&[
        ((mask >> 24) & 0xFF) as u8,
        ((mask >> 16) & 0xFF) as u8,
        ((mask >> 8) & 0xFF) as u8,
        (mask & 0xFF) as u8,
    ]);
}

/// 从 IP CIDR 填充广播地址到字节缓冲区
fn fill_sockaddr_broadcast(buf: &mut [u8], ip_cidr: &smoltcp::wire::IpCidr) {
    use smoltcp::wire::IpAddress;

    buf[0..2].copy_from_slice(&AF_INET.to_ne_bytes());
    buf[2..4].copy_from_slice(&0u16.to_ne_bytes()); // port = 0
    buf[8..16].fill(0); // sin_zero

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

            buf[4..8].copy_from_slice(&broadcast.to_be_bytes());
        }
        _ => {
            buf[4..8].copy_from_slice(&[255, 255, 255, 255]);
        }
    }
}

// 释放获取网络接口列表分配的内存
pub fn freeifaddrs(ifa: *mut u8) -> isize {
    use crate::kernel::syscall::mm::munmap;
    use crate::uapi::errno::EINVAL;

    if ifa.is_null() {
        return 0; // NULL 指针，直接返回
    }

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

    let header: IfAddrsAllocHeader = read_from_user(header_addr as *const IfAddrsAllocHeader);
    if header.magic != IFADDRS_ALLOC_MAGIC || header.map_len < IFADDRS_HEADER_SIZE {
        return -(EINVAL as isize);
    }

    let result = munmap(header_addr as *mut core::ffi::c_void, header.map_len);
    if result < 0 {
        return -(EINVAL as isize);
    }

    0
}
