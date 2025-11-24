# 网络子系统实现指南

## 文档概述

本文档提供了实现完整网络功能的详细指南，包括修复当前问题和实现真正的 POSIX 网络系统调用。

**创建日期**: 2025-11-24
**状态**: 设计文档 / 实现待完成

---

## 1. 当前状态与问题总结

### 1.1 已修复的问题 ✓

#### 问题 1: `create_smoltcp_interface` 内存安全问题
**严重程度**: 🔴 严重 - 未定义行为

**问题描述**:
```rust
// src/device/net/interface.rs (旧代码)
pub fn create_smoltcp_interface(&self) -> Interface {
    let mut device_adapter = NetDeviceAdapter::new(self.device.clone());
    let iface = Interface::new(config, &mut device_adapter, timestamp);
    iface  // ❌ 返回持有悬垂指针的 Interface！
}
```

`device_adapter` 在栈上创建，函数返回后被销毁，导致返回的 `Interface` 持有悬垂指针。

**解决方案**:
创建 `SmoltcpInterface` 包装器，确保 Device 和 Interface 有相同的生命周期：

```rust
pub struct SmoltcpInterface {
    device_adapter: NetDeviceAdapter,  // 拥有 Device
    iface: Interface,                   // Interface 借用 device_adapter
}

impl SmoltcpInterface {
    fn new(device: Arc<dyn NetDevice>, mac_address: EthernetAddress) -> Self {
        let mut device_adapter = NetDeviceAdapter::new(device);
        let iface = Interface::new(config, &mut device_adapter, timestamp);
        Self { device_adapter, iface }
    }

    pub fn poll(&mut self, timestamp: Instant, sockets: &mut SocketSet) -> PollResult {
        self.iface.poll(timestamp, &mut self.device_adapter, sockets)
    }

    pub fn interface_mut(&mut self) -> &mut Interface { &mut self.iface }
    pub fn interface(&self) -> &Interface { &self.iface }
}

// 现在返回包装器而不是裸 Interface
pub fn create_smoltcp_interface(&self) -> SmoltcpInterface {
    let mut smoltcp_iface = SmoltcpInterface::new(self.device.clone(), self.mac_address());
    // ... 配置 IP 和路由
    smoltcp_iface
}
```

**文件位置**: `os/src/device/net/interface.rs:47-99`

---

#### 问题 2: 子网掩码解析功能有限
**严重程度**: 🟡 中等 - 功能受限

**问题描述**:
`set_interface_config` 函数使用硬编码的 match 语句解析子网掩码，只支持 9 种常见掩码。

```rust
// 旧代码
let prefix_length = match mask {
    "255.255.255.0" => 24,
    "255.255.0.0" => 16,
    // ... 仅支持少数几种
    _ => return Err(NetworkConfigError::InvalidSubnet),
};
```

**解决方案**:
实现通用的子网掩码解析函数，支持任意有效掩码：

```rust
/// 解析点分十进制子网掩码并计算前缀长度
///
/// # 算法
/// 1. 解析为 4 字节并转换为 u32
/// 2. 计算前导 1 的个数（前缀长度）
/// 3. 验证掩码有效性：所有 1 必须连续
///    - 有效: 11111111111111111111111100000000 (0xFFFFFF00)
///    - 无效: 11111111111111110000000011111111 (0xFFFF00FF)
///
/// # 示例
/// - "255.255.255.0" → Ok(24)
/// - "255.255.255.128" → Ok(25)
/// - "255.255.255.3" → Err (不连续)
fn parse_subnet_mask(mask: &str) -> Result<u8, NetworkConfigError> {
    // 解析为 4 字节
    let octets: Result<Vec<u8>, _> = mask.split('.').map(|s| s.parse()).collect();
    let octets = octets.map_err(|_| NetworkConfigError::InvalidSubnet)?;

    if octets.len() != 4 {
        return Err(NetworkConfigError::InvalidSubnet);
    }

    // 转换为 u32
    let mask_u32 = ((octets[0] as u32) << 24)
        | ((octets[1] as u32) << 16)
        | ((octets[2] as u32) << 8)
        | (octets[3] as u32);

    // 计算前缀长度
    let prefix_length = mask_u32.leading_ones() as u8;

    // 验证掩码连续性
    if prefix_length == 0 {
        if mask_u32 == 0 { Ok(0) } else { Err(NetworkConfigError::InvalidSubnet) }
    } else if prefix_length == 32 {
        if mask_u32 == 0xFFFFFFFF { Ok(32) } else { Err(NetworkConfigError::InvalidSubnet) }
    } else {
        let expected_mask = 0xFFFFFFFFu32 << (32 - prefix_length);
        if mask_u32 == expected_mask {
            Ok(prefix_length)
        } else {
            Err(NetworkConfigError::InvalidSubnet)
        }
    }
}

// 使用
let prefix_length = Self::parse_subnet_mask(mask)?;
```

**文件位置**: `os/src/device/net/config.rs:20-87, 240`

---

### 1.2 未修复的问题 - 网络系统调用存根

#### 问题 3: 所有网络系统调用只是存根实现
**严重程度**: 🔴 严重 - 核心功能缺失

**问题描述**:
所有网络系统调用 (`socket`, `bind`, `listen`, `accept`, `connect`, `send`, `recv` 等) 都只返回虚拟值，没有实现真正的网络功能。

**当前实现** (`os/src/kernel/syscall/net_syscall.rs`):
```rust
pub fn socket(domain: i32, socket_type: i32, protocol: i32) -> isize {
    // TODO: 实现套接字创建
    3  // ❌ 返回虚拟 FD
}

pub fn bind(sockfd: i32, addr: *const u8, addrlen: u32) -> isize {
    // TODO: 实现绑定逻辑
    0  // ❌ 假装成功
}

pub fn send(sockfd: i32, buf: *const u8, len: usize, flags: i32) -> isize {
    // TODO: 实现发送逻辑
    len as isize  // ❌ 假装发送了所有数据
}

pub fn recv(sockfd: i32, buf: *mut u8, len: usize, flags: i32) -> isize {
    // TODO: 实现接收逻辑
    0  // ❌ 总是返回没有数据
}
```

**影响**:
- 用户程序无法使用网络功能
- 与 POSIX 标准不兼容
- 无法运行真实的网络应用

---

## 2. 完整网络功能架构设计

### 2.1 整体架构图

```
┌─────────────────────────────────────────────────────────────┐
│                    用户空间                                   │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │  TCP 应用   │  │  UDP 应用   │  │  原始套接字  │          │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘          │
└─────────┼─────────────────┼─────────────────┼───────────────┘
          │                 │                 │
          │    POSIX Socket API (syscall)    │
          │                 │                 │
┌─────────┼─────────────────┼─────────────────┼───────────────┐
│         │       内核空间                    │                 │
│  ┌──────▼──────────────────▼─────────────▼──────┐           │
│  │          网络系统调用层                       │           │
│  │  socket/bind/listen/accept/connect/send/recv  │           │
│  └──────┬──────────────────────────────────┬─────┘           │
│         │                                   │                 │
│  ┌──────▼───────────────────────────────────▼─────┐          │
│  │           Socket 文件抽象层                     │          │
│  │  SocketFile (实现 File trait)                  │          │
│  │  - TcpSocketFile                                │          │
│  │  - UdpSocketFile                                │          │
│  └──────┬──────────────────────────────────┬──────┘          │
│         │                                   │                 │
│  ┌──────▼───────────────────────────────────▼──────┐         │
│  │          网络协议栈管理器                        │         │
│  │  NetworkStack                                    │         │
│  │  - SmoltcpInterface (设备 + 接口)               │         │
│  │  - SocketSet (所有 socket 的集合)               │         │
│  │  - Socket 元数据表                              │         │
│  └──────┬────────────────────────────────────────┘          │
│         │                                                     │
│  ┌──────▼────────────────────────────────────────┐          │
│  │          smoltcp 协议栈                        │          │
│  │  - TCP/UDP/IP/ICMP 协议实现                    │          │
│  │  - Socket 管理                                 │          │
│  └──────┬─────────────────────────────────────────┘          │
│         │                                                     │
│  ┌──────▼────────────────────────────────────────┐          │
│  │      NetDeviceAdapter (设备适配器)             │          │
│  └──────┬─────────────────────────────────────────┘          │
│         │                                                     │
│  ┌──────▼────────────────────────────────────────┐          │
│  │         VirtIO 网络驱动                        │          │
│  │  (VirtioNet)                                   │          │
│  └────────────────────────────────────────────────┘          │
│                                                               │
└───────────────────────────────────────────────────────────────┘
                           │
                  ┌────────▼────────┐
                  │  网络硬件 (QEMU) │
                  └─────────────────┘
```

### 2.2 关键组件说明

#### 2.2.1 Socket 文件抽象 (`os/src/vfs/socket.rs` - 需要创建)

**目的**: 将 socket 集成到 VFS 中，使其像文件一样可以通过 FD 访问。

```rust
use crate::vfs::File;
use alloc::sync::Arc;
use smoltcp::socket::{TcpSocket, UdpSocket};
use smoltcp::wire::{IpEndpoint, IpAddress};

/// Socket 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    Stream,   // TCP (SOCK_STREAM)
    Datagram, // UDP (SOCK_DGRAM)
    Raw,      // 原始套接字 (SOCK_RAW)
}

/// Socket 地址
#[derive(Debug, Clone, Copy)]
pub struct SocketAddr {
    pub ip: IpAddress,
    pub port: u16,
}

impl SocketAddr {
    pub fn to_endpoint(&self) -> IpEndpoint {
        IpEndpoint::new(self.ip, self.port)
    }
}

/// Socket 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    Closed,
    Listening,
    Connecting,
    Connected,
    FinWait,
}

/// TCP Socket 文件
pub struct TcpSocketFile {
    /// smoltcp socket handle
    socket_handle: SocketHandle,

    /// Socket 状态
    state: SpinLock<SocketState>,

    /// 本地绑定地址
    local_addr: SpinLock<Option<SocketAddr>>,

    /// 远程连接地址
    remote_addr: SpinLock<Option<SocketAddr>>,

    /// 等待队列（用于 accept 的连接队列）
    pending_connections: SpinLock<VecDeque<SocketHandle>>,

    /// 最大等待连接数（backlog）
    backlog: usize,
}

impl TcpSocketFile {
    pub fn new(socket_handle: SocketHandle) -> Self {
        Self {
            socket_handle,
            state: SpinLock::new(SocketState::Closed),
            local_addr: SpinLock::new(None),
            remote_addr: SpinLock::new(None),
            pending_connections: SpinLock::new(VecDeque::new()),
            backlog: 0,
        }
    }

    /// 绑定到本地地址
    pub fn bind(&self, addr: SocketAddr) -> Result<(), NetworkError> {
        // 实现绑定逻辑
        todo!()
    }

    /// 监听连接
    pub fn listen(&self, backlog: usize) -> Result<(), NetworkError> {
        // 实现监听逻辑
        todo!()
    }

    /// 接受连接
    pub fn accept(&self) -> Result<(Arc<TcpSocketFile>, SocketAddr), NetworkError> {
        // 实现 accept 逻辑
        todo!()
    }

    /// 连接到远程地址
    pub fn connect(&self, addr: SocketAddr) -> Result<(), NetworkError> {
        // 实现连接逻辑
        todo!()
    }
}

/// 为 TcpSocketFile 实现 File trait
impl File for TcpSocketFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize, FileError> {
        // 从 TCP socket 读取数据
        // 需要访问全局 NetworkStack 来操作 socket
        todo!()
    }

    fn write(&self, buf: &[u8]) -> Result<usize, FileError> {
        // 向 TCP socket 写入数据
        todo!()
    }

    fn seek(&self, _pos: SeekFrom) -> Result<u64, FileError> {
        // Socket 不支持 seek
        Err(FileError::NotSupported)
    }

    fn is_seekable(&self) -> bool {
        false
    }

    // ... 其他 File trait 方法
}

/// UDP Socket 文件
pub struct UdpSocketFile {
    socket_handle: SocketHandle,
    local_addr: SpinLock<Option<SocketAddr>>,
    remote_addr: SpinLock<Option<SocketAddr>>,
}

impl UdpSocketFile {
    pub fn new(socket_handle: SocketHandle) -> Self {
        Self {
            socket_handle,
            local_addr: SpinLock::new(None),
            remote_addr: SpinLock::new(None),
        }
    }

    pub fn bind(&self, addr: SocketAddr) -> Result<(), NetworkError> {
        todo!()
    }

    pub fn sendto(&self, buf: &[u8], addr: SocketAddr) -> Result<usize, NetworkError> {
        todo!()
    }

    pub fn recvfrom(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), NetworkError> {
        todo!()
    }
}

impl File for UdpSocketFile {
    // 实现类似 TcpSocketFile 的方法
    // ...
}

/// 网络错误
#[derive(Debug)]
pub enum NetworkError {
    InvalidAddress,
    InvalidSocket,
    NotConnected,
    AlreadyConnected,
    ConnectionRefused,
    WouldBlock,
    Timeout,
    // ... 其他错误
}
```

#### 2.2.2 网络协议栈管理器 (`os/src/net/stack.rs` - 需要创建)

**目的**: 管理全局的 smoltcp 协议栈实例、socket 集合和元数据。

```rust
use crate::device::net::interface::SmoltcpInterface;
use crate::sync::SpinLock;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use lazy_static::lazy_static;
use smoltcp::socket::{SocketHandle, SocketSet, TcpSocket, UdpSocket};
use smoltcp::time::Instant;

/// Socket 元数据
pub struct SocketMetadata {
    pub socket_type: SocketType,
    pub local_addr: Option<SocketAddr>,
    pub remote_addr: Option<SocketAddr>,
    pub state: SocketState,
}

/// 全局网络协议栈
pub struct NetworkStack {
    /// smoltcp 接口（包含 Device 和 Interface）
    smoltcp_iface: SpinLock<SmoltcpInterface>,

    /// Socket 集合（所有 socket 的容器）
    socket_set: SpinLock<SocketSet<'static>>,

    /// Socket 元数据映射表
    /// SocketHandle -> SocketMetadata
    socket_metadata: SpinLock<BTreeMap<SocketHandle, SocketMetadata>>,

    /// 当前时间（用于 smoltcp）
    current_time: SpinLock<Instant>,
}

impl NetworkStack {
    pub fn new(smoltcp_iface: SmoltcpInterface) -> Self {
        Self {
            smoltcp_iface: SpinLock::new(smoltcp_iface),
            socket_set: SpinLock::new(SocketSet::new(Vec::new())),
            socket_metadata: SpinLock::new(BTreeMap::new()),
            current_time: SpinLock::new(Instant::from_millis(0)),
        }
    }

    /// 创建新的 TCP socket
    pub fn create_tcp_socket(&self) -> Result<SocketHandle, NetworkError> {
        let mut socket_set = self.socket_set.lock();

        // 创建 TCP socket 缓冲区
        let tcp_rx_buffer = TcpSocketBuffer::new(vec![0; 4096]);
        let tcp_tx_buffer = TcpSocketBuffer::new(vec![0; 4096]);
        let tcp_socket = TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);

        // 添加到 socket 集合
        let socket_handle = socket_set.add(tcp_socket);

        // 记录元数据
        let mut metadata = self.socket_metadata.lock();
        metadata.insert(socket_handle, SocketMetadata {
            socket_type: SocketType::Stream,
            local_addr: None,
            remote_addr: None,
            state: SocketState::Closed,
        });

        Ok(socket_handle)
    }

    /// 创建新的 UDP socket
    pub fn create_udp_socket(&self) -> Result<SocketHandle, NetworkError> {
        let mut socket_set = self.socket_set.lock();

        // 创建 UDP socket 缓冲区
        let udp_rx_buffer = UdpSocketBuffer::new(
            vec![UdpPacketMetadata::EMPTY; 16],
            vec![0; 4096]
        );
        let udp_tx_buffer = UdpSocketBuffer::new(
            vec![UdpPacketMetadata::EMPTY; 16],
            vec![0; 4096]
        );
        let udp_socket = UdpSocket::new(udp_rx_buffer, udp_tx_buffer);

        let socket_handle = socket_set.add(udp_socket);

        let mut metadata = self.socket_metadata.lock();
        metadata.insert(socket_handle, SocketMetadata {
            socket_type: SocketType::Datagram,
            local_addr: None,
            remote_addr: None,
            state: SocketState::Closed,
        });

        Ok(socket_handle)
    }

    /// 删除 socket
    pub fn remove_socket(&self, handle: SocketHandle) {
        let mut socket_set = self.socket_set.lock();
        socket_set.remove(handle);

        let mut metadata = self.socket_metadata.lock();
        metadata.remove(&handle);
    }

    /// 绑定 TCP socket 到本地地址
    pub fn tcp_bind(&self, handle: SocketHandle, addr: SocketAddr) -> Result<(), NetworkError> {
        let mut socket_set = self.socket_set.lock();
        let socket = socket_set.get_mut::<TcpSocket>(handle);

        // 调用 smoltcp 的 listen
        socket.listen(addr.port)
            .map_err(|_| NetworkError::InvalidAddress)?;

        // 更新元数据
        let mut metadata = self.socket_metadata.lock();
        if let Some(meta) = metadata.get_mut(&handle) {
            meta.local_addr = Some(addr);
        }

        Ok(())
    }

    /// TCP connect
    pub fn tcp_connect(
        &self,
        handle: SocketHandle,
        remote_addr: SocketAddr,
        local_port: u16,
    ) -> Result<(), NetworkError> {
        let mut socket_set = self.socket_set.lock();
        let socket = socket_set.get_mut::<TcpSocket>(handle);

        socket.connect(
            self.smoltcp_iface.lock().interface().context(),
            remote_addr.to_endpoint(),
            local_port,
        ).map_err(|_| NetworkError::ConnectionRefused)?;

        let mut metadata = self.socket_metadata.lock();
        if let Some(meta) = metadata.get_mut(&handle) {
            meta.remote_addr = Some(remote_addr);
            meta.state = SocketState::Connecting;
        }

        Ok(())
    }

    /// TCP send
    pub fn tcp_send(&self, handle: SocketHandle, data: &[u8]) -> Result<usize, NetworkError> {
        let mut socket_set = self.socket_set.lock();
        let socket = socket_set.get_mut::<TcpSocket>(handle);

        if !socket.can_send() {
            return Err(NetworkError::WouldBlock);
        }

        socket.send_slice(data)
            .map_err(|_| NetworkError::WouldBlock)
    }

    /// TCP recv
    pub fn tcp_recv(&self, handle: SocketHandle, buffer: &mut [u8]) -> Result<usize, NetworkError> {
        let mut socket_set = self.socket_set.lock();
        let socket = socket_set.get_mut::<TcpSocket>(handle);

        if !socket.can_recv() {
            return Err(NetworkError::WouldBlock);
        }

        socket.recv_slice(buffer)
            .map_err(|_| NetworkError::WouldBlock)
    }

    /// UDP sendto
    pub fn udp_sendto(
        &self,
        handle: SocketHandle,
        data: &[u8],
        remote_addr: SocketAddr,
    ) -> Result<(), NetworkError> {
        let mut socket_set = self.socket_set.lock();
        let socket = socket_set.get_mut::<UdpSocket>(handle);

        socket.send_slice(data, remote_addr.to_endpoint())
            .map_err(|_| NetworkError::WouldBlock)
    }

    /// UDP recvfrom
    pub fn udp_recvfrom(
        &self,
        handle: SocketHandle,
        buffer: &mut [u8],
    ) -> Result<(usize, SocketAddr), NetworkError> {
        let mut socket_set = self.socket_set.lock();
        let socket = socket_set.get_mut::<UdpSocket>(handle);

        match socket.recv_slice(buffer) {
            Ok((size, endpoint)) => {
                let addr = SocketAddr {
                    ip: endpoint.addr,
                    port: endpoint.port,
                };
                Ok((size, addr))
            }
            Err(_) => Err(NetworkError::WouldBlock),
        }
    }

    /// 轮询网络栈（处理所有网络事件）
    /// 应该在定时器中断或专门的网络线程中定期调用
    pub fn poll(&self) {
        // 更新时间
        let mut current_time = self.current_time.lock();
        *current_time = Instant::from_millis(current_time.total_millis() + 10);

        // 轮询接口
        let mut socket_set = self.socket_set.lock();
        let mut smoltcp_iface = self.smoltcp_iface.lock();

        smoltcp_iface.poll(*current_time, &mut socket_set);
    }
}

lazy_static! {
    /// 全局网络协议栈实例
    pub static ref NETWORK_STACK: SpinLock<Option<Arc<NetworkStack>>> =
        SpinLock::new(None);
}

/// 初始化网络协议栈
pub fn init_network_stack(smoltcp_iface: SmoltcpInterface) {
    let stack = Arc::new(NetworkStack::new(smoltcp_iface));
    *NETWORK_STACK.lock() = Some(stack);
}

/// 获取全局网络协议栈
pub fn get_network_stack() -> Option<Arc<NetworkStack>> {
    NETWORK_STACK.lock().clone()
}
```

#### 2.2.3 网络系统调用实现 (`os/src/kernel/syscall/net_syscall.rs` - 需要重写)

**目的**: 实现真正的 POSIX socket 系统调用。

```rust
use crate::net::stack::{get_network_stack, NetworkStack};
use crate::net::socket::{SocketAddr, TcpSocketFile, UdpSocketFile};
use crate::kernel::task::current_task;
use crate::vfs::File;
use alloc::sync::Arc;
use core::ffi::c_void;

/// sys_socket - 创建套接字
///
/// # 参数
/// - domain: AF_INET (2) / AF_INET6 (10)
/// - type: SOCK_STREAM (1) / SOCK_DGRAM (2) / SOCK_RAW (3)
/// - protocol: 0 (自动选择)
///
/// # 返回值
/// - 成功: 文件描述符
/// - 失败: 负数错误码
pub fn sys_socket(domain: i32, socket_type: i32, protocol: i32) -> isize {
    // 验证参数
    if domain != 2 {  // AF_INET
        return -97;  // -EAFNOSUPPORT
    }

    let stack = match get_network_stack() {
        Some(s) => s,
        None => return -19,  // -ENODEV (网络栈未初始化)
    };

    // 根据 socket 类型创建不同的 socket
    let socket_file: Arc<dyn File> = match socket_type {
        1 => {  // SOCK_STREAM (TCP)
            let handle = match stack.create_tcp_socket() {
                Ok(h) => h,
                Err(_) => return -12,  // -ENOMEM
            };
            Arc::new(TcpSocketFile::new(handle))
        }
        2 => {  // SOCK_DGRAM (UDP)
            let handle = match stack.create_udp_socket() {
                Ok(h) => h,
                Err(_) => return -12,  // -ENOMEM
            };
            Arc::new(UdpSocketFile::new(handle))
        }
        _ => return -93,  // -EPROTONOSUPPORT
    };

    // 将 socket 添加到当前任务的文件描述符表
    let task = current_task().unwrap();
    let fd = match task.add_file(socket_file) {
        Ok(fd) => fd,
        Err(_) => return -24,  // -EMFILE (太多打开的文件)
    };

    fd as isize
}

/// sys_bind - 绑定套接字到地址
///
/// # 参数
/// - sockfd: socket 文件描述符
/// - addr: sockaddr 结构指针
/// - addrlen: 地址长度
///
/// # 返回值
/// - 成功: 0
/// - 失败: 负数错误码
pub fn sys_bind(sockfd: i32, addr: *const u8, addrlen: u32) -> isize {
    if sockfd < 0 || addr.is_null() {
        return -14;  // -EFAULT
    }

    // 解析 sockaddr_in 结构
    let socket_addr = unsafe {
        if addrlen < 16 {  // sizeof(sockaddr_in)
            return -22;  // -EINVAL
        }

        // sockaddr_in 结构:
        // - u16 sin_family
        // - u16 sin_port (网络字节序)
        // - u32 sin_addr (网络字节序)
        let port = u16::from_be((addr.add(2) as *const u16).read());
        let ip_bytes = core::slice::from_raw_parts(addr.add(4), 4);
        let ip = IpAddress::v4(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);

        SocketAddr { ip, port }
    };

    // 获取 socket 文件
    let task = current_task().unwrap();
    let socket_file = match task.get_file(sockfd as usize) {
        Some(f) => f,
        None => return -9,  // -EBADF
    };

    // 尝试向下转型为 TcpSocketFile 或 UdpSocketFile
    if let Some(tcp_socket) = socket_file.as_any().downcast_ref::<TcpSocketFile>() {
        match tcp_socket.bind(socket_addr) {
            Ok(_) => 0,
            Err(_) => -98,  // -EADDRINUSE
        }
    } else if let Some(udp_socket) = socket_file.as_any().downcast_ref::<UdpSocketFile>() {
        match udp_socket.bind(socket_addr) {
            Ok(_) => 0,
            Err(_) => -98,  // -EADDRINUSE
        }
    } else {
        -88  // -ENOTSOCK
    }
}

/// sys_listen - 监听连接
pub fn sys_listen(sockfd: i32, backlog: i32) -> isize {
    if sockfd < 0 {
        return -9;  // -EBADF
    }

    let task = current_task().unwrap();
    let socket_file = match task.get_file(sockfd as usize) {
        Some(f) => f,
        None => return -9,
    };

    // 只有 TCP socket 支持 listen
    if let Some(tcp_socket) = socket_file.as_any().downcast_ref::<TcpSocketFile>() {
        match tcp_socket.listen(backlog as usize) {
            Ok(_) => 0,
            Err(_) => -22,  // -EINVAL
        }
    } else {
        -95  // -EOPNOTSUPP (操作不支持)
    }
}

/// sys_accept - 接受连接
pub fn sys_accept(sockfd: i32, addr: *mut u8, addrlen: *mut u32) -> isize {
    if sockfd < 0 {
        return -9;
    }

    let task = current_task().unwrap();
    let socket_file = match task.get_file(sockfd as usize) {
        Some(f) => f,
        None => return -9,
    };

    let tcp_socket = match socket_file.as_any().downcast_ref::<TcpSocketFile>() {
        Some(s) => s,
        None => return -88,  // -ENOTSOCK
    };

    // 接受连接
    let (new_socket_file, remote_addr) = match tcp_socket.accept() {
        Ok(result) => result,
        Err(_) => return -11,  // -EAGAIN (没有连接可接受)
    };

    // 填充地址信息
    if !addr.is_null() && !addrlen.is_null() {
        unsafe {
            let available_len = *addrlen as usize;
            if available_len >= 16 {
                // 填充 sockaddr_in
                // ... (类似 bind 的逆操作)
            }
        }
    }

    // 为新连接创建 FD
    match task.add_file(new_socket_file) {
        Ok(fd) => fd as isize,
        Err(_) => -24,  // -EMFILE
    }
}

/// sys_connect - 连接到远程地址
pub fn sys_connect(sockfd: i32, addr: *const u8, addrlen: u32) -> isize {
    if sockfd < 0 || addr.is_null() {
        return -14;
    }

    // 解析地址（类似 bind）
    let socket_addr = unsafe {
        // ... 解析 sockaddr_in
    };

    let task = current_task().unwrap();
    let socket_file = match task.get_file(sockfd as usize) {
        Some(f) => f,
        None => return -9,
    };

    let tcp_socket = match socket_file.as_any().downcast_ref::<TcpSocketFile>() {
        Some(s) => s,
        None => return -88,
    };

    match tcp_socket.connect(socket_addr) {
        Ok(_) => 0,
        Err(_) => -111,  // -ECONNREFUSED
    }
}

/// sys_send / sys_sendto - 发送数据
pub fn sys_sendto(
    sockfd: i32,
    buf: *const u8,
    len: usize,
    flags: i32,
    dest_addr: *const u8,
    addrlen: u32,
) -> isize {
    if sockfd < 0 || buf.is_null() || len == 0 {
        return -14;
    }

    let task = current_task().unwrap();
    let socket_file = match task.get_file(sockfd as usize) {
        Some(f) => f,
        None => return -9,
    };

    // 从用户空间拷贝数据
    let data = unsafe { core::slice::from_raw_parts(buf, len) };

    // TCP socket
    if let Some(tcp_socket) = socket_file.as_any().downcast_ref::<TcpSocketFile>() {
        match socket_file.write(data) {
            Ok(written) => written as isize,
            Err(_) => -11,  // -EAGAIN
        }
    }
    // UDP socket
    else if let Some(udp_socket) = socket_file.as_any().downcast_ref::<UdpSocketFile>() {
        if dest_addr.is_null() {
            return -89;  // -EDESTADDRREQ
        }

        let remote_addr = unsafe {
            // 解析 dest_addr
        };

        match udp_socket.sendto(data, remote_addr) {
            Ok(sent) => sent as isize,
            Err(_) => -11,
        }
    } else {
        -88  // -ENOTSOCK
    }
}

/// sys_recv / sys_recvfrom - 接收数据
pub fn sys_recvfrom(
    sockfd: i32,
    buf: *mut u8,
    len: usize,
    flags: i32,
    src_addr: *mut u8,
    addrlen: *mut u32,
) -> isize {
    if sockfd < 0 || buf.is_null() || len == 0 {
        return -14;
    }

    let task = current_task().unwrap();
    let socket_file = match task.get_file(sockfd as usize) {
        Some(f) => f,
        None => return -9,
    };

    let buffer = unsafe { core::slice::from_raw_parts_mut(buf, len) };

    // TCP socket
    if let Some(_tcp_socket) = socket_file.as_any().downcast_ref::<TcpSocketFile>() {
        match socket_file.read(buffer) {
            Ok(read) => read as isize,
            Err(_) => 0,  // 没有数据可读
        }
    }
    // UDP socket
    else if let Some(udp_socket) = socket_file.as_any().downcast_ref::<UdpSocketFile>() {
        match udp_socket.recvfrom(buffer) {
            Ok((size, remote_addr)) => {
                // 填充源地址
                if !src_addr.is_null() && !addrlen.is_null() {
                    unsafe {
                        // 填充 sockaddr_in
                    }
                }
                size as isize
            }
            Err(_) => 0,
        }
    } else {
        -88
    }
}

// ... 其他系统调用 (close, shutdown, getsockopt, setsockopt 等)
```

---

## 3. 实现步骤

### 第 1 步: 创建 Socket 文件抽象
**文件**: `os/src/vfs/socket.rs`

1. 定义 `SocketType`, `SocketAddr`, `SocketState` 等基础类型
2. 实现 `TcpSocketFile` 结构体及其方法
3. 实现 `UdpSocketFile` 结构体及其方法
4. 为两者实现 `File` trait
5. 在 `os/src/vfs/mod.rs` 中导出

**测试**: 编译通过，类型检查正确

---

### 第 2 步: 创建网络协议栈管理器
**文件**: `os/src/net/stack.rs` (需要先创建 `os/src/net/` 目录)

1. 定义 `NetworkStack` 结构体
2. 实现 socket 创建/删除方法
3. 实现 TCP/UDP 操作方法 (bind, connect, send, recv 等)
4. 实现 `poll()` 方法
5. 创建全局实例和初始化函数

**测试**: 编译通过，可以创建 NetworkStack 实例

---

### 第 3 步: 修改 NetworkInterface 初始化
**文件**: `os/src/device/net/interface.rs`, `os/src/main.rs` 或初始化代码

1. 在网络设备初始化时创建 `SmoltcpInterface`
2. 使用 `SmoltcpInterface` 初始化 `NetworkStack`
3. 设置全局 `NETWORK_STACK`

**代码示例**:
```rust
// 在网络初始化函数中
pub fn init_network() {
    // ... 初始化网络接口
    let manager = NETWORK_INTERFACE_MANAGER.lock();
    if let Some(iface) = manager.get_interfaces().first() {
        let smoltcp_iface = iface.create_smoltcp_interface();
        init_network_stack(smoltcp_iface);
    }
}
```

---

### 第 4 步: 实现网络系统调用
**文件**: `os/src/kernel/syscall/net_syscall.rs`

1. 重写 `sys_socket`
2. 重写 `sys_bind`
3. 重写 `sys_listen`
4. 重写 `sys_accept`
5. 重写 `sys_connect`
6. 重写 `sys_sendto` / `sys_send`
7. 重写 `sys_recvfrom` / `sys_recv`
8. 实现其他系统调用 (close, shutdown, getsockopt, setsockopt)

**测试**: 逐个测试每个系统调用

---

### 第 5 步: 实现网络轮询机制
**选项 A: 定时器中断轮询**
```rust
// 在定时器中断处理函数中
pub fn timer_interrupt_handler() {
    // ... 其他定时器逻辑

    if let Some(stack) = get_network_stack() {
        stack.poll();  // 轮询网络栈
    }

    // ... 调度等
}
```

**选项 B: 专门的网络线程** (推荐)
```rust
// 创建内核线程
pub fn network_polling_thread() {
    loop {
        if let Some(stack) = get_network_stack() {
            stack.poll();
        }

        // 休眠一小段时间（如 10ms）
        sleep_ms(10);
    }
}
```

---

### 第 6 步: 处理阻塞和非阻塞
1. 为 socket 添加阻塞/非阻塞标志
2. 阻塞模式下，recv/send 应该等待数据或空间可用
3. 实现等待队列，让任务在 socket 上等待
4. 网络事件到达时唤醒等待的任务

---

### 第 7 步: 测试
1. **基础测试**: 创建 socket, bind, close
2. **UDP 测试**: 发送和接收 UDP 数据包
3. **TCP 客户端测试**: 连接到外部服务器，发送/接收数据
4. **TCP 服务器测试**: 监听端口，接受连接
5. **并发测试**: 多个连接同时工作
6. **错误处理测试**: 各种错误情况

**测试用户程序示例**:
```c
// user/src/test_tcp_client.c
int main() {
    int sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd < 0) {
        printf("socket() failed\n");
        return 1;
    }

    struct sockaddr_in server_addr = {
        .sin_family = AF_INET,
        .sin_port = htons(80),
        .sin_addr.s_addr = inet_addr("192.168.1.1"),
    };

    if (connect(sockfd, (struct sockaddr*)&server_addr, sizeof(server_addr)) < 0) {
        printf("connect() failed\n");
        return 1;
    }

    const char* request = "GET / HTTP/1.0\r\n\r\n";
    send(sockfd, request, strlen(request), 0);

    char buffer[1024];
    int n = recv(sockfd, buffer, sizeof(buffer), 0);
    if (n > 0) {
        printf("Received: %.*s\n", n, buffer);
    }

    close(sockfd);
    return 0;
}
```

---

## 4. 关键注意事项

### 4.1 线程安全
- 所有全局状态都必须用锁保护
- 避免死锁：定义锁的获取顺序
- 注意：不能在中断处理程序中调用可能阻塞的操作

### 4.2 内存管理
- smoltcp 的 socket 缓冲区需要静态生命周期
- 使用 `Vec` 或 `Box` 分配堆内存
- 注意避免内存泄漏（socket 关闭时释放资源）

### 4.3 地址字节序
- 网络字节序是大端 (big-endian)
- RISC-V 通常是小端 (little-endian)
- 使用 `u16::from_be()` / `u16::to_be()` 转换

### 4.4 用户空间内存访问
- 所有用户指针都必须验证
- 使用 `copy_from_user` / `copy_to_user` 安全拷贝
- 防止用户传入内核地址

### 4.5 错误处理
- 使用标准的 POSIX 错误码 (errno)
- 常见错误码:
  - `-EBADF` (9): 错误的文件描述符
  - `-EINVAL` (22): 无效参数
  - `-EAGAIN` (11): 资源暂时不可用
  - `-ECONNREFUSED` (111): 连接被拒绝
  - `-EADDRINUSE` (98): 地址已被使用

---

## 5. 依赖和模块关系

### 5.1 新增模块依赖图
```
vfs/socket.rs
    ├─> sync::SpinLock
    ├─> vfs::File (trait)
    ├─> net::stack::NetworkStack
    └─> smoltcp::socket::{TcpSocket, UdpSocket}

net/stack.rs
    ├─> sync::SpinLock
    ├─> device::net::interface::SmoltcpInterface
    ├─> smoltcp::socket::SocketSet
    └─> smoltcp::time::Instant

kernel/syscall/net_syscall.rs
    ├─> vfs::socket::{TcpSocketFile, UdpSocketFile}
    ├─> net::stack::{get_network_stack, NetworkStack}
    ├─> kernel::task::current_task
    └─> 用户内存访问函数
```

### 5.2 遵循模块分层
根据 CLAUDE.md 中的模块层次规则：
```
arch → mm → sync → kernel → {ipc, vfs, fs, net}
```

- `net/` 模块位于最上层，可以使用所有下层模块
- 不能让下层模块依赖 `net/`
- Socket 作为 VFS 的一部分，位于 `vfs/socket.rs`

---

## 6. 性能优化建议（后续）

1. **零拷贝**: 使用 DMA 直接在网卡和用户空间之间传输
2. **批量处理**: 一次轮询处理多个数据包
3. **中断合并**: 减少中断频率
4. **Socket 缓冲区调优**: 根据应用调整缓冲区大小
5. **连接复用**: 支持 SO_REUSEADDR 和 SO_REUSEPORT

---

## 7. 未来扩展

### 7.1 IPv6 支持
- 扩展 `SocketAddr` 支持 IPv6
- 处理 `AF_INET6` domain

### 7.2 原始套接字 (SOCK_RAW)
- 允许直接访问 IP/ICMP 层
- 需要权限检查

### 7.3 Unix Domain Socket
- 进程间通信
- 文件系统路径作为地址

### 7.4 高级功能
- `poll()` / `epoll()` 系统调用
- `sendmsg()` / `recvmsg()` (scatter-gather I/O)
- `sendfile()` (零拷贝文件传输)
- Socket 选项 (SO_KEEPALIVE, TCP_NODELAY 等)

---

## 8. 参考资料

### 8.1 文档
- [smoltcp 文档](https://docs.rs/smoltcp/)
- [POSIX Socket API](https://pubs.opengroup.org/onlinepubs/9699919799/functions/socket.html)
- [Linux Socket Man Pages](https://man7.org/linux/man-pages/man2/socket.2.html)

### 8.2 代码示例
- [Redox OS 网络实现](https://gitlab.redox-os.org/redox-os/netstack)
- [rCore 网络模块](https://github.com/rcore-os/rCore/tree/master/kernel/src/net)

---

## 9. 检查清单

实现完成后，确保：

- [ ] 所有 TODO 注释已删除
- [ ] 所有系统调用都返回正确的错误码
- [ ] 内存安全（无悬垂指针、无内存泄漏）
- [ ] 线程安全（所有共享状态都有锁保护）
- [ ] 用户输入验证（防止越界、空指针等）
- [ ] 编写了测试用户程序
- [ ] 通过了所有测试
- [ ] 更新了相关文档
- [ ] 代码符合项目风格规范（`cargo fmt`, `make quick_check_style`）

---

## 附录 A: smoltcp Socket 缓冲区大小建议

```rust
// TCP
let tcp_rx_buffer = TcpSocketBuffer::new(vec![0; 8192]);   // 8KB 接收缓冲区
let tcp_tx_buffer = TcpSocketBuffer::new(vec![0; 8192]);   // 8KB 发送缓冲区

// UDP
let udp_rx_buffer = UdpSocketBuffer::new(
    vec![UdpPacketMetadata::EMPTY; 16],  // 最多 16 个数据包
    vec![0; 4096]                         // 4KB 总缓冲区
);
let udp_tx_buffer = UdpSocketBuffer::new(
    vec![UdpPacketMetadata::EMPTY; 16],
    vec![0; 4096]
);
```

根据应用需求调整：
- **Web 服务器**: 增大 TCP 缓冲区 (16KB - 64KB)
- **音视频流**: 增大 UDP 缓冲区
- **嵌入式/内存受限**: 减小缓冲区 (1KB - 2KB)

---

## 附录 B: 常见错误码对照表

| 错误名          | 值  | 含义                   | 返回时机                    |
|-----------------|-----|------------------------|-----------------------------|
| EBADF           | 9   | 错误的文件描述符       | FD 无效或不是 socket        |
| EAGAIN/EWOULDBLOCK | 11 | 资源暂时不可用        | 非阻塞操作会阻塞            |
| ENOMEM          | 12  | 内存不足               | 无法分配 socket             |
| EFAULT          | 14  | 错误的地址             | 用户指针无效                |
| EINVAL          | 22  | 无效参数               | 参数检查失败                |
| EMFILE          | 24  | 打开文件过多           | FD 表已满                   |
| ENOTSOCK        | 88  | 不是 socket            | 对非 socket FD 操作         |
| EDESTADDRREQ    | 89  | 需要目标地址           | UDP sendto 未提供地址       |
| EPROTONOSUPPORT | 93  | 不支持的协议           | 协议参数错误                |
| EOPNOTSUPP      | 95  | 不支持的操作           | 对 UDP socket 调用 listen   |
| EAFNOSUPPORT    | 97  | 不支持的地址族         | domain 不是 AF_INET         |
| EADDRINUSE      | 98  | 地址已被使用           | bind 到已占用端口           |
| ECONNREFUSED    | 111 | 连接被拒绝             | connect 失败                |

---

**文档维护**: 实现过程中遇到问题或有新发现时，请更新此文档。
