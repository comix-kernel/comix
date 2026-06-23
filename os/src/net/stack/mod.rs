//! Network stack runtime facade.
//!
//! This module owns the protocol-stack runtime state and operations.

use crate::sync::SpinLock;
use alloc::collections::{BTreeMap, VecDeque};
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use smoltcp::iface::{Interface, SocketHandle as SmoltcpHandle, SocketSet};
use smoltcp::socket::{tcp, udp};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, IpAddress, IpEndpoint, IpListenEndpoint, Ipv4Address};

use super::NetworkError;
use super::socket::{self, SocketFile, SocketHandle, UdpDatagram};

mod adapter;
pub use adapter::{NetDeviceAdapter, SmoltcpInterface};

const TCP_RX_BUFFER_SIZE: usize = 256 * 1024;
const TCP_TX_BUFFER_SIZE: usize = 256 * 1024;
const UDP_PACKET_METADATA_CAPACITY: usize = 256;
const UDP_RX_BUFFER_SIZE: usize = 256 * 1024;
const UDP_TX_BUFFER_SIZE: usize = 256 * 1024;
const LOOPBACK_WRITE_DRAIN_POLLS: usize = 64;
const LOOPBACK_FULL_DRAIN_POLLS: usize = 256;

pub(crate) fn enqueue_loopback_frame(frame: Vec<u8>) {
    network_stack().enqueue_loopback_frame(frame);
}

fn dequeue_loopback_frame() -> Option<Vec<u8>> {
    network_stack().dequeue_loopback_frame()
}

pub(crate) fn loopback_link_len() -> usize {
    network_stack().loopback_link_len()
}

/// Active smoltcp interface runtime.
pub struct NetIfaceWrapper {
    device: SpinLock<NetDeviceAdapter>,
    interface: SpinLock<Interface>,
}

impl NetIfaceWrapper {
    fn poll_smoltcp(&self, sockets: &SpinLock<SocketSet<'static>>) -> bool {
        let timestamp = smoltcp::time::Instant::from_millis(crate::arch::get_time_ms() as i64);
        let mut dev = self.device.lock();

        let queue_len = dev.loopback_queue_len();
        if queue_len > 0 {
            crate::pr_debug!("poll: loopback queue has {} packets", queue_len);
        }

        let mut iface = self.interface.lock();
        let mut sockets = sockets.lock();

        crate::pr_debug!("poll: before iface.poll");
        let result = iface.poll(timestamp, &mut *dev, &mut sockets);
        crate::pr_debug!("poll: result={:?}", result);

        // Frames produced by loopback Tx are visible to Rx on a later poll.
        if dev.loopback_queue_len() > 0 {
            const MAX_EXTRA_POLLS: usize = 2;
            for _ in 0..MAX_EXTRA_POLLS {
                if dev.loopback_queue_len() == 0 {
                    break;
                }
                let _ = iface.poll(timestamp, &mut *dev, &mut sockets);
            }
        }

        result != smoltcp::iface::PollResult::None
    }

    fn loopback_queue_len(&self) -> usize {
        self.device.lock().loopback_queue_len()
    }

    fn with_context<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut smoltcp::iface::Context) -> R,
    {
        let mut iface = self.interface.lock();
        f(iface.context())
    }
}

struct UdpPortEntry {
    handle: SmoltcpHandle,
    sockets: alloc::vec::Vec<alloc::sync::Weak<dyn crate::vfs::File>>,
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

/// Stateful network protocol-stack runtime.
pub struct NetworkStack {
    socket_set: SpinLock<SocketSet<'static>>,
    net_iface: SpinLock<Option<NetIfaceWrapper>>,
    loopback_link: SpinLock<VecDeque<Vec<u8>>>,
    udp_ports: SpinLock<BTreeMap<u16, UdpPortEntry>>,
    pending_tcp_close: SpinLock<alloc::vec::Vec<SmoltcpHandle>>,
}

impl NetworkStack {
    fn new() -> Self {
        Self {
            socket_set: SpinLock::new(SocketSet::new(alloc::vec![])),
            net_iface: SpinLock::new(None),
            loopback_link: SpinLock::new(VecDeque::new()),
            udp_ports: SpinLock::new(BTreeMap::new()),
            pending_tcp_close: SpinLock::new(alloc::vec::Vec::new()),
        }
    }

    fn enqueue_loopback_frame(&self, frame: Vec<u8>) {
        self.loopback_link.lock().push_back(frame);
    }

    fn dequeue_loopback_frame(&self) -> Option<Vec<u8>> {
        self.loopback_link.lock().pop_front()
    }

    fn loopback_link_len(&self) -> usize {
        self.loopback_link.lock().len()
    }

    /// Initialize the active smoltcp interface runtime.
    pub fn init_network(&self, mut smoltcp_iface: crate::net::interface::SmoltcpInterface) {
        let wrapper = NetIfaceWrapper {
            device: SpinLock::new(smoltcp_iface.device_adapter_mut().clone()),
            interface: SpinLock::new(smoltcp_iface.into_interface()),
        };
        *self.net_iface.lock() = Some(wrapper);
    }

    /// Create a TCP socket in the stack runtime.
    pub fn create_tcp_socket(&self) -> Result<SocketHandle, NetworkError> {
        let mut rx_vec = alloc::vec::Vec::new();
        rx_vec
            .try_reserve(TCP_RX_BUFFER_SIZE)
            .map_err(|_| NetworkError::NoMemory)?;
        rx_vec.resize(TCP_RX_BUFFER_SIZE, 0);

        let mut tx_vec = alloc::vec::Vec::new();
        tx_vec
            .try_reserve(TCP_TX_BUFFER_SIZE)
            .map_err(|_| NetworkError::NoMemory)?;
        tx_vec.resize(TCP_TX_BUFFER_SIZE, 0);

        let rx_buffer = tcp::SocketBuffer::new(rx_vec);
        let tx_buffer = tcp::SocketBuffer::new(tx_vec);
        let socket = tcp::Socket::new(rx_buffer, tx_buffer);

        let handle = self.socket_set.lock().add(socket);
        Ok(SocketHandle::Tcp(handle))
    }

    /// Create a UDP socket in the stack runtime.
    pub fn create_udp_socket(&self) -> Result<SocketHandle, NetworkError> {
        let mut sockets = self.socket_set.lock();
        let handle = self.create_udp_socket_in_set(&mut sockets)?;
        Ok(SocketHandle::Udp(handle))
    }

    /// Connect a TCP socket using the active interface context.
    pub fn tcp_connect(
        &self,
        handle: SmoltcpHandle,
        remote: IpEndpoint,
        local: IpEndpoint,
    ) -> Result<(), NetworkError> {
        crate::pr_debug!("tcp_connect: start, handle={:?}", handle);

        let iface_guard = self.net_iface.lock();
        crate::pr_debug!("tcp_connect: got net_iface lock");
        let wrapper = iface_guard.as_ref().ok_or(NetworkError::NotInitialized)?;

        let result = wrapper.with_context(|context| {
            crate::pr_debug!("tcp_connect: in with_context");
            let mut sockets = self.socket_set.lock();
            crate::pr_debug!("tcp_connect: got socket_set lock");
            let socket = sockets.get_mut::<tcp::Socket>(handle);
            crate::pr_debug!("tcp_connect: calling socket.connect");
            let r = socket.connect(context, remote, local).map_err(|e| {
                crate::pr_debug!("tcp_connect error: {:?}", e);
                NetworkError::ConnectFailed
            });
            crate::pr_debug!("tcp_connect: socket.connect returned {:?}", r);
            r
        });

        if result.is_ok() {
            crate::pr_debug!("tcp_connect: polling to send SYN");
            wrapper.poll_smoltcp(&self.socket_set);
        }

        drop(iface_guard);
        crate::pr_debug!("tcp_connect: done, result={:?}", result);
        result
    }

    /// Query the coarse TCP state without exposing the smoltcp socket set to callers.
    pub fn tcp_connection_state(&self, handle: SmoltcpHandle) -> Option<TcpConnectionState> {
        let sockets = self.socket_set.lock();
        let socket = sockets.get::<tcp::Socket>(handle);
        Some(match socket.state() {
            tcp::State::Established => TcpConnectionState::Established,
            tcp::State::Closed => TcpConnectionState::Closed,
            _ => TcpConnectionState::Other,
        })
    }

    /// Start listening on a TCP socket.
    pub fn tcp_listen(
        &self,
        handle: SmoltcpHandle,
        endpoint: IpListenEndpoint,
    ) -> Result<(), NetworkError> {
        let mut sockets = self.socket_set.lock();
        sockets
            .get_mut::<tcp::Socket>(handle)
            .listen(endpoint)
            .map_err(|_| NetworkError::AddressInUse)
    }

    /// Query listener state and endpoint without exposing `SocketSet`.
    pub fn tcp_listener_state_endpoint(
        &self,
        handle: SmoltcpHandle,
    ) -> Option<(TcpListenState, IpListenEndpoint)> {
        let sockets = self.socket_set.lock();
        let socket = sockets.get::<tcp::Socket>(handle);
        let state = match socket.state() {
            tcp::State::Listen => TcpListenState::Listen,
            tcp::State::Established => TcpListenState::Established,
            tcp::State::CloseWait => TcpListenState::CloseWait,
            _ => TcpListenState::Other,
        };
        Some((state, socket.listen_endpoint()))
    }

    /// Remove a TCP socket handle from the runtime.
    pub fn remove_tcp_socket(&self, handle: SmoltcpHandle) {
        self.socket_set.lock().remove(handle);
    }

    /// Return remote and local endpoints for an accepted TCP socket.
    pub fn tcp_accept_endpoints(
        &self,
        handle: SmoltcpHandle,
    ) -> Option<(IpEndpoint, Option<IpEndpoint>)> {
        let sockets = self.socket_set.lock();
        let socket = sockets.get::<tcp::Socket>(handle);
        let remote = socket.remote_endpoint()?;
        Some((remote, socket.local_endpoint()))
    }

    /// Return lightweight TCP debug state for syscall logging.
    pub fn tcp_debug_state(&self, handle: SmoltcpHandle) -> Option<(TcpConnectionState, bool)> {
        let sockets = self.socket_set.lock();
        let socket = sockets.get::<tcp::Socket>(handle);
        let state = match socket.state() {
            tcp::State::Established => TcpConnectionState::Established,
            tcp::State::Closed => TcpConnectionState::Closed,
            _ => TcpConnectionState::Other,
        };
        Some((state, socket.is_open()))
    }

    /// Close a TCP socket.
    pub fn tcp_close(&self, handle: SmoltcpHandle) {
        self.socket_set
            .lock()
            .get_mut::<tcp::Socket>(handle)
            .close();
    }

    /// Query a socket's local endpoint.
    pub fn socket_local_endpoint(&self, handle: SocketHandle) -> Option<IpEndpoint> {
        let sockets = self.socket_set.lock();
        match handle {
            SocketHandle::Tcp(h) => sockets.get::<tcp::Socket>(h).local_endpoint(),
            SocketHandle::Udp(h) => {
                let listen_ep = sockets.get::<udp::Socket>(h).endpoint();
                Some(IpEndpoint::new(
                    listen_ep
                        .addr
                        .unwrap_or(IpAddress::Ipv4(Ipv4Address::UNSPECIFIED)),
                    listen_ep.port,
                ))
            }
        }
    }

    /// Query a socket's remote endpoint.
    pub fn socket_remote_endpoint(&self, handle: SocketHandle) -> Option<IpEndpoint> {
        let sockets = self.socket_set.lock();
        match handle {
            SocketHandle::Tcp(h) => sockets.get::<tcp::Socket>(h).remote_endpoint(),
            SocketHandle::Udp(_) => None,
        }
    }

    /// Poll the network stack once.
    pub fn poll(&self) {
        if let Some(ref wrapper) = *self.net_iface.lock() {
            crate::pr_debug!("poll_network_interfaces: calling poll");
            let smoltcp_changed = wrapper.poll_smoltcp(&self.socket_set);
            let udp_changed = {
                let mut sockets = self.socket_set.lock();
                self.udp_dispatch_drain_locked(&mut sockets)
            };
            self.reap_pending_tcp_close();
            if smoltcp_changed || udp_changed {
                crate::kernel::syscall::io::wake_poll_waiters();
            }
        }
    }

    /// Poll smoltcp and dispatch queued datagrams.
    pub fn poll_and_dispatch(&self) {
        self.poll();
    }

    /// Drain UDP datagrams from shared per-port sockets.
    pub fn udp_dispatch(&self) -> bool {
        let mut sockets = self.socket_set.lock();
        self.udp_dispatch_drain_locked(&mut sockets)
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
    ) -> Result<SmoltcpHandle, NetworkError> {
        let shared_handle = {
            let mut sockets = self.socket_set.lock();
            let mut ports = self.udp_ports.lock();
            if let Some(e) = ports.get(&port) {
                e.handle
            } else {
                let h = self.create_udp_socket_in_set(&mut sockets)?;
                let listen = IpListenEndpoint {
                    addr: bind_addr,
                    port,
                };
                if sockets.get_mut::<udp::Socket>(h).bind(listen).is_err() {
                    sockets.remove(h);
                    return Err(NetworkError::AddressInUse);
                }
                ports.insert(port, UdpPortEntry {
                    handle: h,
                    sockets: alloc::vec::Vec::new(),
                });
                h
            }
        };

        socket::update_socket_handle(tid, fd, SocketHandle::Udp(shared_handle));
        if let Some(sf) = file.as_any().downcast_ref::<SocketFile>() {
            sf.set_handle(SocketHandle::Udp(shared_handle));
        }

        {
            let mut ports = self.udp_ports.lock();
            if let Some(e) = ports.get_mut(&port) {
                let already = e
                    .sockets
                    .iter()
                    .filter_map(|w| w.upgrade())
                    .any(|f| alloc::sync::Arc::ptr_eq(&f, file));
                if !already {
                    e.sockets.push(alloc::sync::Arc::downgrade(file));
                }
            }
        }

        if old_handle != shared_handle {
            let mut sockets = self.socket_set.lock();
            let ports = self.udp_ports.lock();
            let old_is_shared = ports.values().any(|e| e.handle == old_handle);
            drop(ports);
            if !old_is_shared {
                sockets.remove(old_handle);
            }
        }

        Ok(shared_handle)
    }

    /// Poll until loopback compatibility queues are boundedly drained.
    pub fn poll_until_empty(&self) {
        self.poll_loopback_bounded(LOOPBACK_FULL_DRAIN_POLLS);
    }

    fn poll_loopback_bounded(&self, max_polls: usize) {
        if let Some(ref wrapper) = *self.net_iface.lock() {
            wrapper.poll_smoltcp(&self.socket_set);
            {
                let mut sockets = self.socket_set.lock();
                self.udp_dispatch_drain_locked(&mut sockets);
            }
            self.reap_pending_tcp_close();

            for _ in 0..max_polls {
                if wrapper.loopback_queue_len() == 0 {
                    break;
                }
                wrapper.poll_smoltcp(&self.socket_set);
                let mut sockets = self.socket_set.lock();
                self.udp_dispatch_drain_locked(&mut sockets);
            }
        }
    }

    /// Pop an established child socket from a listener's accept queue.
    pub fn take_established_from_listen_queue(
        &self,
        file: &super::socket::SocketFile,
    ) -> Option<SocketHandle> {
        let sockets = self.socket_set.lock();
        let mut q = file.listen_sockets.lock();
        let mut i = 0;
        while i < q.len() {
            match q[i] {
                SocketHandle::Tcp(h) => {
                    let s = sockets.get::<tcp::Socket>(h);
                    match s.state() {
                        tcp::State::Established | tcp::State::CloseWait => {
                            return Some(q.remove(i));
                        }
                        tcp::State::Closed => {
                            q.remove(i);
                            continue;
                        }
                        _ => {}
                    }
                }
                SocketHandle::Udp(_) => {
                    q.remove(i);
                    continue;
                }
            }
            i += 1;
        }
        None
    }

    /// Query whether a socket has reached the closed TCP state.
    pub fn socket_is_closed(&self, file: &super::socket::SocketFile) -> bool {
        let sockets = self.socket_set.lock();
        match file.handle.lock().as_ref() {
            Some(SocketHandle::Tcp(h)) => {
                let socket = sockets.get::<tcp::Socket>(*h);
                socket.state() == tcp::State::Closed
            }
            _ => false,
        }
    }

    /// Release all stack resources owned by a SocketFile.
    pub fn drop_socket_file(&self, file: &super::socket::SocketFile) {
        let mut sockets = self.socket_set.lock();
        if let Some(handle) = *file.handle.lock() {
            match handle {
                SocketHandle::Tcp(h) => {
                    let socket = sockets.get_mut::<tcp::Socket>(h);
                    let state = socket.state();
                    crate::pr_debug!("[Socket] Drop: handle={:?}, state={:?}", h, state);
                    match state {
                        tcp::State::Closed | tcp::State::TimeWait | tcp::State::Listen => {
                            sockets.remove(h);
                        }
                        _ => {
                            crate::pr_debug!("[Socket] Drop: closing socket handle={:?}", h);
                            socket.close();
                            self.pending_tcp_close.lock().push(h);
                        }
                    }
                }
                SocketHandle::Udp(h) => {
                    let ports = self.udp_ports.lock();
                    let is_shared = ports.values().any(|e| e.handle == h);
                    drop(ports);
                    if !is_shared {
                        sockets.remove(h);
                    }
                }
            }
        }
        for handle in file.listen_sockets.lock().iter() {
            match handle {
                SocketHandle::Tcp(h) => {
                    sockets.remove(*h);
                }
                SocketHandle::Udp(h) => {
                    sockets.remove(*h);
                }
            }
        }
    }

    /// Send a datagram to a specific endpoint.
    pub fn socket_sendto(
        &self,
        handle: SocketHandle,
        buf: &[u8],
        endpoint: IpEndpoint,
    ) -> Result<usize, crate::vfs::FsError> {
        let result = {
            let mut sockets = self.socket_set.lock();
            match handle {
                SocketHandle::Tcp(_) => Err(crate::vfs::FsError::NotSupported),
                SocketHandle::Udp(h) => {
                    let socket = sockets.get_mut::<udp::Socket>(h);
                    socket
                        .send_slice(buf, endpoint)
                        .map_err(|_| crate::vfs::FsError::WouldBlock)?;
                    Ok(buf.len())
                }
            }
        };
        if result.is_ok() {
            self.poll_loopback_bounded(LOOPBACK_WRITE_DRAIN_POLLS);
            crate::kernel::syscall::io::wake_poll_waiters();
        }
        result
    }

    /// Query VFS readability for a socket file.
    pub fn socket_readable(&self, file: &super::socket::SocketFile) -> bool {
        if *file.is_listener.lock() {
            let sockets = self.socket_set.lock();

            for handle in file.listen_sockets.lock().iter() {
                if let SocketHandle::Tcp(h) = handle {
                    let s = sockets.get::<tcp::Socket>(*h);
                    if matches!(s.state(), tcp::State::Established | tcp::State::CloseWait) {
                        return true;
                    }
                }
            }

            if let Some(SocketHandle::Tcp(h)) = *file.handle.lock() {
                let s = sockets.get::<tcp::Socket>(h);
                return matches!(s.state(), tcp::State::Established | tcp::State::CloseWait);
            }
            return false;
        }

        let sockets = self.socket_set.lock();
        match file.handle.lock().as_ref() {
            Some(SocketHandle::Tcp(h)) => {
                let socket = sockets.get::<tcp::Socket>(*h);
                let can_recv = socket.can_recv();
                let state = socket.state();
                crate::pr_debug!(
                    "[Socket] readable: handle={:?}, state={:?}, can_recv={}",
                    h,
                    state,
                    can_recv
                );
                can_recv || matches!(state, tcp::State::Closed | tcp::State::CloseWait)
            }
            Some(SocketHandle::Udp(_)) => {
                drop(sockets);
                file.udp_queue_len() > 0
            }
            None => false,
        }
    }

    /// Query VFS writability for a socket file.
    pub fn socket_writable(&self, file: &super::socket::SocketFile) -> bool {
        let sockets = self.socket_set.lock();
        match file.handle.lock().as_ref() {
            Some(SocketHandle::Tcp(h)) => {
                let socket = sockets.get::<tcp::Socket>(*h);
                let can_send = socket.can_send();
                let state = socket.state();
                crate::pr_debug!(
                    "[Socket] writable: handle={:?}, state={:?}, can_send={}",
                    h,
                    state,
                    can_send
                );
                can_send
            }
            Some(SocketHandle::Udp(h)) => {
                let socket = sockets.get::<udp::Socket>(*h);
                socket.can_send()
            }
            None => false,
        }
    }

    /// Read stream data or a queued UDP datagram payload.
    pub fn socket_read(
        &self,
        file: &super::socket::SocketFile,
        buf: &mut [u8],
    ) -> Result<usize, crate::vfs::FsError> {
        if file.is_shutdown_read() {
            return Ok(0);
        }

        let mut sockets = self.socket_set.lock();
        let result = match file.handle.lock().as_ref() {
            Some(SocketHandle::Tcp(h)) => {
                let socket = sockets.get_mut::<tcp::Socket>(*h);
                let state = socket.state();
                let recv_queue = socket.recv_queue();
                crate::pr_debug!(
                    "[Socket] read: handle={:?}, state={:?}, recv_queue={}, buf.len()={}",
                    h,
                    state,
                    recv_queue,
                    buf.len()
                );

                if socket.state() == tcp::State::Closed {
                    return Ok(0);
                }

                if state == tcp::State::CloseWait && recv_queue == 0 {
                    return Ok(0);
                }

                let result = socket
                    .recv_slice(buf)
                    .map_err(|_| crate::vfs::FsError::WouldBlock);

                if let Ok(0) = result {
                    if state == tcp::State::CloseWait {
                        crate::pr_debug!(
                            "[Socket] read: recv_slice returned 0 and state=CloseWait, returning EOF"
                        );
                        Ok(0)
                    } else if socket.may_recv() {
                        crate::pr_debug!(
                            "[Socket] read: recv_slice returned 0 but may_recv=true, returning EAGAIN"
                        );
                        Err(crate::vfs::FsError::WouldBlock)
                    } else {
                        crate::pr_debug!(
                            "[Socket] read: recv_slice returned 0 and may_recv=false, returning EOF"
                        );
                        Ok(0)
                    }
                } else {
                    if let Ok(n) = result {
                        crate::pr_debug!("[Socket] read: received {} bytes", n);
                    }
                    result
                }
            }
            Some(SocketHandle::Udp(_)) => {
                drop(sockets);
                let Some(d) = file.udp_pop() else {
                    return Err(crate::vfs::FsError::WouldBlock);
                };
                let n = core::cmp::min(buf.len(), d.len);
                buf[..n].copy_from_slice(&d.data[..n]);
                Ok(n)
            }
            None => Err(crate::vfs::FsError::InvalidArgument),
        };
        if result.is_ok() {
            crate::kernel::syscall::io::wake_poll_waiters();
        }
        result
    }

    /// Write stream data or a connected UDP datagram payload.
    pub fn socket_write(
        &self,
        file: &super::socket::SocketFile,
        buf: &[u8],
    ) -> Result<usize, crate::vfs::FsError> {
        if file.is_shutdown_write() {
            return Err(crate::vfs::FsError::BrokenPipe);
        }

        let result = {
            let mut sockets = self.socket_set.lock();
            match file.handle.lock().as_ref() {
                Some(SocketHandle::Tcp(h)) => {
                    let socket = sockets.get_mut::<tcp::Socket>(*h);
                    let result = socket
                        .send_slice(buf)
                        .map_err(|_| crate::vfs::FsError::WouldBlock);

                    if !buf.is_empty()
                        && let Ok(0) = result
                    {
                        if socket.may_send() {
                            return Err(crate::vfs::FsError::WouldBlock);
                        } else {
                            return Err(crate::vfs::FsError::BrokenPipe);
                        }
                    }

                    result
                }
                Some(SocketHandle::Udp(h)) => {
                    let endpoint = match file.get_remote_endpoint() {
                        Some(ep) => ep,
                        None => return Err(crate::vfs::FsError::NotConnected),
                    };
                    let socket = sockets.get_mut::<udp::Socket>(*h);
                    socket
                        .send_slice(buf, endpoint)
                        .map_err(|_| crate::vfs::FsError::WouldBlock)?;
                    Ok(buf.len())
                }
                None => Err(crate::vfs::FsError::InvalidArgument),
            }
        };
        if result.is_ok() {
            self.poll_loopback_bounded(LOOPBACK_WRITE_DRAIN_POLLS);
            crate::kernel::syscall::io::wake_poll_waiters();
        }
        result
    }

    /// Receive data and, when available, the peer sockaddr buffer.
    pub fn socket_recvfrom(
        &self,
        file: &super::socket::SocketFile,
        buf: &mut [u8],
    ) -> Result<(usize, Option<alloc::vec::Vec<u8>>), crate::vfs::FsError> {
        if file.is_shutdown_read() {
            return Ok((0, None));
        }

        let mut sockets = self.socket_set.lock();
        match file.handle.lock().as_ref() {
            Some(SocketHandle::Tcp(h)) => {
                let socket = sockets.get_mut::<tcp::Socket>(*h);
                let state = socket.state();
                if state == tcp::State::Closed {
                    return Ok((0, None));
                }
                if state == tcp::State::CloseWait && socket.recv_queue() == 0 {
                    return Ok((0, None));
                }

                let result = socket
                    .recv_slice(buf)
                    .map_err(|_| crate::vfs::FsError::WouldBlock);
                let n = if let Ok(0) = result {
                    if state == tcp::State::CloseWait {
                        0
                    } else if socket.may_recv() {
                        return Err(crate::vfs::FsError::WouldBlock);
                    } else {
                        0
                    }
                } else {
                    result?
                };
                let remote = socket.remote_endpoint().map(|ep| {
                    let mut addr_buf = alloc::vec![0u8; 16];
                    let _ = socket::write_sockaddr_in_to_buf(&mut addr_buf, ep);
                    addr_buf
                });
                Ok((n, remote))
            }
            Some(SocketHandle::Udp(_)) => {
                drop(sockets);
                let Some(d) = file.udp_pop() else {
                    return Err(crate::vfs::FsError::WouldBlock);
                };
                let n = core::cmp::min(buf.len(), d.len);
                buf[..n].copy_from_slice(&d.data[..n]);
                let mut addr_buf = alloc::vec![0u8; 16];
                let _ = socket::write_sockaddr_in_to_buf(&mut addr_buf, d.src);
                Ok((n, Some(addr_buf)))
            }
            None => Err(crate::vfs::FsError::InvalidArgument),
        }
    }

    fn create_udp_socket_in_set(
        &self,
        sockets: &mut SocketSet<'static>,
    ) -> Result<SmoltcpHandle, NetworkError> {
        let mut rx_meta_vec = alloc::vec::Vec::new();
        rx_meta_vec
            .try_reserve(UDP_PACKET_METADATA_CAPACITY)
            .map_err(|_| NetworkError::NoMemory)?;
        rx_meta_vec.resize(UDP_PACKET_METADATA_CAPACITY, udp::PacketMetadata::EMPTY);

        let mut tx_meta_vec = alloc::vec::Vec::new();
        tx_meta_vec
            .try_reserve(UDP_PACKET_METADATA_CAPACITY)
            .map_err(|_| NetworkError::NoMemory)?;
        tx_meta_vec.resize(UDP_PACKET_METADATA_CAPACITY, udp::PacketMetadata::EMPTY);

        let mut rx_data_vec = alloc::vec::Vec::new();
        rx_data_vec
            .try_reserve(UDP_RX_BUFFER_SIZE)
            .map_err(|_| NetworkError::NoMemory)?;
        rx_data_vec.resize(UDP_RX_BUFFER_SIZE, 0);

        let mut tx_data_vec = alloc::vec::Vec::new();
        tx_data_vec
            .try_reserve(UDP_TX_BUFFER_SIZE)
            .map_err(|_| NetworkError::NoMemory)?;
        tx_data_vec.resize(UDP_TX_BUFFER_SIZE, 0);

        let rx_buffer = udp::PacketBuffer::new(rx_meta_vec, rx_data_vec);
        let tx_buffer = udp::PacketBuffer::new(tx_meta_vec, tx_data_vec);
        let socket = udp::Socket::new(rx_buffer, tx_buffer);
        Ok(sockets.add(socket))
    }

    fn udp_dispatch_drain_locked(&self, sockets: &mut SocketSet<'static>) -> bool {
        let mut delivered_any = false;
        let mut ports = self.udp_ports.lock();
        let mut to_remove: alloc::vec::Vec<(u16, SmoltcpHandle)> = alloc::vec::Vec::new();

        for (port, entry) in ports.iter_mut() {
            let socket = sockets.get_mut::<udp::Socket>(entry.handle);

            while socket.can_recv() {
                let (payload, meta) = match socket.recv() {
                    Ok(v) => v,
                    Err(_) => break,
                };

                let src = meta.endpoint;
                let mut data = [0u8; socket::UDP_DGRAM_MAX];
                let copy_len = core::cmp::min(payload.len(), socket::UDP_DGRAM_MAX);
                data[..copy_len].copy_from_slice(&payload[..copy_len]);
                let d = UdpDatagram {
                    src,
                    len: copy_len,
                    data,
                };

                let mut target: Option<alloc::sync::Arc<dyn crate::vfs::File>> = None;
                let mut fallback: Option<alloc::sync::Arc<dyn crate::vfs::File>> = None;

                entry.sockets.retain(|w| w.strong_count() > 0);

                for w in entry.sockets.iter() {
                    let Some(f) = w.upgrade() else { continue };
                    let Some(sf) = f.as_any().downcast_ref::<SocketFile>() else {
                        continue;
                    };

                    let Some(local_ep) = sf.get_local_endpoint() else {
                        continue;
                    };
                    if local_ep.port != *port {
                        continue;
                    }

                    match sf.get_remote_endpoint() {
                        Some(remote) => {
                            if remote.addr == src.addr && remote.port == src.port {
                                target = Some(f.clone());
                                break;
                            }
                        }
                        None => {
                            if fallback.is_none() {
                                fallback = Some(f.clone());
                            }
                        }
                    }
                }

                let target = target.or(fallback);
                if let Some(f) = target
                    && let Some(sf) = f.as_any().downcast_ref::<SocketFile>()
                    && sf.udp_push(d)
                {
                    delivered_any = true;
                }
            }

            entry.sockets.retain(|w| w.strong_count() > 0);
            if entry.sockets.is_empty() {
                to_remove.push((*port, entry.handle));
            }
        }

        for (port, handle) in to_remove {
            ports.remove(&port);
            sockets.remove(handle);
        }

        delivered_any
    }

    fn reap_pending_tcp_close(&self) {
        let mut sockets = self.socket_set.lock();
        let mut pending = self.pending_tcp_close.lock();
        pending.retain(|h| {
            let state = sockets.get::<tcp::Socket>(*h).state();
            if matches!(state, tcp::State::Closed | tcp::State::TimeWait) {
                crate::pr_debug!(
                    "[Socket] reap: removing closed tcp handle={:?}, state={:?}",
                    h,
                    state
                );
                sockets.remove(*h);
                false
            } else {
                true
            }
        });
    }
}

lazy_static! {
    static ref NETWORK_STACK: NetworkStack = NetworkStack::new();
}

/// Borrow the global network stack runtime.
pub fn network_stack() -> &'static NetworkStack {
    &NETWORK_STACK
}
