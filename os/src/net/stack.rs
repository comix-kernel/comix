//! Network stack runtime facade.
//!
//! This module is the migration host for protocol-stack state.  The first
//! refactor step keeps the existing implementation in `socket.rs`, while new
//! call sites start depending on this facade instead of reaching for raw
//! globals such as `SOCKET_SET` or `NET_IFACE`.

use crate::sync::SpinLock;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use smoltcp::iface::{Interface, SocketHandle as SmoltcpHandle, SocketSet};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, IpEndpoint, IpListenEndpoint};

use super::socket::{NetIfaceWrapper, SocketHandle};

lazy_static! {
    /// Stack-owned smoltcp socket set.
    pub(crate) static ref SOCKET_SET: SpinLock<SocketSet<'static>> =
        SpinLock::new(SocketSet::new(alloc::vec![]));

    /// Stack-owned active interface runtime.
    pub(crate) static ref NET_IFACE: SpinLock<Option<NetIfaceWrapper>> = SpinLock::new(None);

    /// Runtime loopback link used while the stack is still single-interface.
    static ref LOOPBACK_LINK: SpinLock<VecDeque<Vec<u8>>> = SpinLock::new(VecDeque::new());
}

pub(crate) fn enqueue_loopback_frame(frame: Vec<u8>) {
    LOOPBACK_LINK.lock().push_back(frame);
}

fn dequeue_loopback_frame() -> Option<Vec<u8>> {
    LOOPBACK_LINK.lock().pop_front()
}

pub(crate) fn loopback_link_len() -> usize {
    LOOPBACK_LINK.lock().len()
}

/// Smoltcp interface wrapper that owns the adapter borrowed by `Interface`.
pub struct SmoltcpInterface {
    device_adapter: NetDeviceAdapter,
    iface: Interface,
}

impl SmoltcpInterface {
    /// Create a smoltcp interface wrapper for a net device.
    pub(crate) fn new(
        device: Arc<dyn crate::device::net::net_device::NetDevice>,
        mac_address: EthernetAddress,
    ) -> Self {
        let mut device_adapter = NetDeviceAdapter::new(device);
        let config =
            smoltcp::iface::Config::new(smoltcp::wire::HardwareAddress::Ethernet(mac_address));
        let current_time = crate::arch::timer::get_time_ms() as i64;
        let iface = Interface::new(
            config,
            &mut device_adapter,
            Instant::from_millis(current_time),
        );

        Self {
            device_adapter,
            iface,
        }
    }

    /// Poll the interface and sockets.
    pub fn poll(
        &mut self,
        timestamp: Instant,
        sockets: &mut smoltcp::iface::SocketSet,
    ) -> smoltcp::iface::PollResult {
        self.iface
            .poll(timestamp, &mut self.device_adapter, sockets)
    }

    /// Mutable access for compatibility during the migration.
    pub fn interface_mut(&mut self) -> &mut Interface {
        &mut self.iface
    }

    /// Immutable access for compatibility during the migration.
    pub fn interface(&self) -> &Interface {
        &self.iface
    }

    /// Mutable adapter access for compatibility during the migration.
    pub fn device_adapter_mut(&mut self) -> &mut NetDeviceAdapter {
        &mut self.device_adapter
    }

    /// Consume the wrapper and return the smoltcp interface.
    pub fn into_interface(self) -> Interface {
        self.iface
    }
}

/// Adapter from Comix `NetDevice` to smoltcp's `Device` trait.
#[derive(Clone)]
pub struct NetDeviceAdapter {
    device: Arc<dyn crate::device::net::net_device::NetDevice>,
    rx_buffer: [u8; 2048],
}

impl NetDeviceAdapter {
    /// Create a new adapter.
    pub fn new(device: Arc<dyn crate::device::net::net_device::NetDevice>) -> Self {
        Self {
            device,
            rx_buffer: [0; 2048],
        }
    }

    /// Compatibility hook for the old bounded loopback drain path.
    pub fn loopback_queue_len(&self) -> usize {
        loopback_link_len()
    }
}

impl smoltcp::phy::Device for NetDeviceAdapter {
    type RxToken<'a> = NetRxToken<'a>;
    type TxToken<'a> = NetTxToken<'a>;

    fn receive(
        &mut self,
        _timestamp: smoltcp::time::Instant,
    ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        if let Some(packet) = dequeue_loopback_frame() {
            if packet.len() > self.rx_buffer.len() {
                return None;
            }
            self.rx_buffer[..packet.len()].copy_from_slice(&packet);
            return Some((
                NetRxToken {
                    buffer: &self.rx_buffer[..packet.len()],
                },
                NetTxToken {
                    device: &self.device,
                },
            ));
        }

        match self.device.receive(&mut self.rx_buffer) {
            Ok(size) if size > 0 => Some((
                NetRxToken {
                    buffer: &self.rx_buffer[..size],
                },
                NetTxToken {
                    device: &self.device,
                },
            )),
            _ => None,
        }
    }

    fn transmit(&mut self, _timestamp: smoltcp::time::Instant) -> Option<Self::TxToken<'_>> {
        Some(NetTxToken {
            device: &self.device,
        })
    }

    fn capabilities(&self) -> smoltcp::phy::DeviceCapabilities {
        let mut caps = smoltcp::phy::DeviceCapabilities::default();
        caps.max_transmission_unit =
            self.device.mtu() + smoltcp::wire::EthernetFrame::<&[u8]>::header_len();
        caps.medium = smoltcp::phy::Medium::Ethernet;
        caps.max_burst_size = Some(64);
        caps
    }
}

/// Receive token.
pub struct NetRxToken<'a> {
    buffer: &'a [u8],
}

impl smoltcp::phy::RxToken for NetRxToken<'_> {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(self.buffer)
    }

    fn meta(&self) -> smoltcp::phy::PacketMeta {
        smoltcp::phy::PacketMeta::default()
    }
}

/// Transmit token.
pub struct NetTxToken<'a> {
    device: &'a Arc<dyn crate::device::net::net_device::NetDevice>,
}

impl smoltcp::phy::TxToken for NetTxToken<'_> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = alloc::vec![0; len];
        let result = f(&mut buffer);

        let is_loopback = if buffer.len() >= 14 {
            let ethertype = u16::from_be_bytes([buffer[12], buffer[13]]);
            match ethertype {
                0x0800 if buffer.len() >= 34 => buffer[26] == 127 || buffer[30] == 127,
                0x0806 if buffer.len() >= 42 => buffer[28] == 127 || buffer[38] == 127,
                _ => false,
            }
        } else {
            false
        };

        if is_loopback {
            enqueue_loopback_frame(buffer);
        } else {
            let _ = self.device.send(&buffer);
        }

        result
    }

    fn set_meta(&mut self, _meta: smoltcp::phy::PacketMeta) {}
}

/// Coarse TCP state exposed to syscall code during the migration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TcpConnectionState {
    /// The connection handshake completed.
    Established,
    /// The socket reached smoltcp's closed state.
    Closed,
    /// Any other in-progress state.
    Other,
}

/// Coarse state for a listening TCP socket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TcpListenState {
    /// The socket is still in listen state.
    Listen,
    /// The socket has an established connection ready to accept.
    Established,
    /// The peer has closed after establishment, still acceptable by userspace.
    CloseWait,
    /// Any other transient state.
    Other,
}

/// Stable facade for network protocol-stack operations.
pub struct NetworkStack;

impl NetworkStack {
    const fn new() -> Self {
        Self
    }

    /// Initialize the active smoltcp interface runtime.
    pub fn init_network(&self, smoltcp_iface: crate::net::interface::SmoltcpInterface) {
        super::socket::init_network_impl(smoltcp_iface);
    }

    /// Create a TCP socket in the stack runtime.
    pub fn create_tcp_socket(&self) -> Result<SocketHandle, ()> {
        super::socket::create_tcp_socket_impl()
    }

    /// Create a UDP socket in the stack runtime.
    pub fn create_udp_socket(&self) -> Result<SocketHandle, ()> {
        super::socket::create_udp_socket_impl()
    }

    /// Connect a TCP socket using the active interface context.
    pub fn tcp_connect(
        &self,
        handle: SmoltcpHandle,
        remote: IpEndpoint,
        local: IpEndpoint,
    ) -> Result<(), ()> {
        super::socket::tcp_connect_impl(handle, remote, local)
    }

    /// Query the coarse TCP state without exposing `SOCKET_SET` to callers.
    pub fn tcp_connection_state(&self, handle: SmoltcpHandle) -> Option<TcpConnectionState> {
        super::socket::tcp_connection_state_impl(handle)
    }

    /// Start listening on a TCP socket.
    pub fn tcp_listen(&self, handle: SmoltcpHandle, endpoint: IpListenEndpoint) -> Result<(), ()> {
        super::socket::tcp_listen_impl(handle, endpoint)
    }

    /// Query listener state and endpoint without exposing `SocketSet`.
    pub fn tcp_listener_state_endpoint(
        &self,
        handle: SmoltcpHandle,
    ) -> Option<(TcpListenState, IpListenEndpoint)> {
        super::socket::tcp_listener_state_endpoint_impl(handle)
    }

    /// Remove a TCP socket handle from the runtime.
    pub fn remove_tcp_socket(&self, handle: SmoltcpHandle) {
        super::socket::remove_tcp_socket_impl(handle);
    }

    /// Return remote and local endpoints for an accepted TCP socket.
    pub fn tcp_accept_endpoints(
        &self,
        handle: SmoltcpHandle,
    ) -> Option<(IpEndpoint, Option<IpEndpoint>)> {
        super::socket::tcp_accept_endpoints_impl(handle)
    }

    /// Return lightweight TCP debug state for syscall logging.
    pub fn tcp_debug_state(&self, handle: SmoltcpHandle) -> Option<(TcpConnectionState, bool)> {
        super::socket::tcp_debug_state_impl(handle)
    }

    /// Close a TCP socket.
    pub fn tcp_close(&self, handle: SmoltcpHandle) {
        super::socket::tcp_close_impl(handle);
    }

    /// Query a socket's local endpoint.
    pub fn socket_local_endpoint(&self, handle: SocketHandle) -> Option<IpEndpoint> {
        super::socket::socket_local_endpoint_impl(handle)
    }

    /// Query a socket's remote endpoint.
    pub fn socket_remote_endpoint(&self, handle: SocketHandle) -> Option<IpEndpoint> {
        super::socket::socket_remote_endpoint_impl(handle)
    }

    /// Poll the network stack once.
    pub fn poll(&self) {
        super::socket::poll_network_interfaces_impl();
    }

    /// Poll smoltcp and dispatch queued datagrams.
    pub fn poll_and_dispatch(&self) {
        super::socket::poll_network_and_dispatch_impl();
    }

    /// Drain UDP datagrams from shared per-port sockets.
    pub fn udp_dispatch(&self) -> bool {
        super::socket::udp_dispatch_impl()
    }

    /// Attach a UDP fd to the shared per-port socket.
    pub fn udp_attach_fd_to_port(
        &self,
        tid: usize,
        fd: usize,
        file: &Arc<dyn crate::vfs::File>,
        old_handle: SmoltcpHandle,
        port: u16,
        bind_addr: Option<smoltcp::wire::IpAddress>,
    ) -> Result<SmoltcpHandle, ()> {
        super::socket::udp_attach_fd_to_port_impl(tid, fd, file, old_handle, port, bind_addr)
    }

    /// Poll until loopback compatibility queues are boundedly drained.
    pub fn poll_until_empty(&self) {
        super::socket::poll_until_empty_impl();
    }

    /// Pop an established child socket from a listener's accept queue.
    pub fn take_established_from_listen_queue(
        &self,
        file: &super::socket::SocketFile,
    ) -> Option<SocketHandle> {
        super::socket::take_established_from_listen_queue_impl(file)
    }

    /// Query whether a socket has reached the closed TCP state.
    pub fn socket_is_closed(&self, file: &super::socket::SocketFile) -> bool {
        super::socket::socket_is_closed_impl(file)
    }

    /// Release all stack resources owned by a SocketFile.
    pub fn drop_socket_file(&self, file: &super::socket::SocketFile) {
        super::socket::drop_socket_file_impl(file);
    }

    /// Send a datagram to a specific endpoint.
    pub fn socket_sendto(
        &self,
        handle: SocketHandle,
        buf: &[u8],
        endpoint: IpEndpoint,
    ) -> Result<usize, crate::vfs::FsError> {
        super::socket::socket_sendto_impl(handle, buf, endpoint)
    }

    /// Query VFS readability for a socket file.
    pub fn socket_readable(&self, file: &super::socket::SocketFile) -> bool {
        super::socket::socket_readable_impl(file)
    }

    /// Query VFS writability for a socket file.
    pub fn socket_writable(&self, file: &super::socket::SocketFile) -> bool {
        super::socket::socket_writable_impl(file)
    }

    /// Read stream data or a queued UDP datagram payload.
    pub fn socket_read(
        &self,
        file: &super::socket::SocketFile,
        buf: &mut [u8],
    ) -> Result<usize, crate::vfs::FsError> {
        super::socket::socket_read_impl(file, buf)
    }

    /// Write stream data or a connected UDP datagram payload.
    pub fn socket_write(
        &self,
        file: &super::socket::SocketFile,
        buf: &[u8],
    ) -> Result<usize, crate::vfs::FsError> {
        super::socket::socket_write_impl(file, buf)
    }

    /// Receive data and, when available, the peer sockaddr buffer.
    pub fn socket_recvfrom(
        &self,
        file: &super::socket::SocketFile,
        buf: &mut [u8],
    ) -> Result<(usize, Option<alloc::vec::Vec<u8>>), crate::vfs::FsError> {
        super::socket::socket_recvfrom_impl(file, buf)
    }
}

lazy_static! {
    static ref NETWORK_STACK: SpinLock<NetworkStack> = SpinLock::new(NetworkStack::new());
}

/// Borrow the global network stack facade.
pub fn network_stack() -> &'static SpinLock<NetworkStack> {
    &NETWORK_STACK
}
