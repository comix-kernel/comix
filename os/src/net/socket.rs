//! Socket implementation using smoltcp

use crate::sync::SpinLock;
use crate::vfs::{File, FsError, InodeMetadata};
use alloc::vec;
use lazy_static::lazy_static;
use smoltcp::iface::{Interface, SocketHandle as SmoltcpHandle, SocketSet};
use smoltcp::socket::{tcp, udp};
use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address};

#[derive(Clone, Copy, Debug)]
pub enum SocketHandle {
    Tcp(SmoltcpHandle),
    Udp(SmoltcpHandle),
}

use alloc::collections::BTreeMap;
use alloc::sync::Arc;

pub struct NetIfaceWrapper {
    device: SpinLock<crate::net::interface::NetDeviceAdapter>,
    interface: SpinLock<Interface>,
}

impl NetIfaceWrapper {
    pub fn poll(&self, sockets: &SpinLock<SocketSet>) -> bool {
        let timestamp = smoltcp::time::Instant::from_millis(
            crate::arch::timer::get_time_ms() as i64
        );
        let mut dev = self.device.lock();

        // 检查队列长度
        let queue_len = dev.loopback_queue_len();
        if queue_len > 0 {
            crate::pr_debug!("poll: loopback queue has {} packets", queue_len);
        }

        let mut iface = self.interface.lock();
        let mut sockets = sockets.lock();

        crate::pr_debug!("poll: before iface.poll");
        let result = iface.poll(timestamp, &mut *dev, &mut *sockets);
        crate::pr_debug!("poll: result={:?}", result);

        // Reap TCP sockets that have finished a graceful close.
        let mut pending = PENDING_TCP_CLOSE.lock();
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

        let changed = result != smoltcp::iface::PollResult::None;
        if changed {
            crate::kernel::syscall::io::wake_poll_waiters();
        }
        changed
    }

    pub fn loopback_queue_len(&self) -> usize {
        self.device.lock().loopback_queue_len()
    }

    pub fn with_context<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut smoltcp::iface::Context) -> R,
    {
        let mut iface = self.interface.lock();
        f(iface.context())
    }
}

lazy_static! {
    pub static ref SOCKET_SET: SpinLock<SocketSet<'static>> = SpinLock::new(SocketSet::new(vec![]));
    pub static ref FD_SOCKET_MAP: SpinLock<BTreeMap<(usize, usize), SocketHandle>> = SpinLock::new(BTreeMap::new());
    pub static ref NET_IFACE: SpinLock<Option<NetIfaceWrapper>> = SpinLock::new(None);
    // TCP sockets that initiated a graceful close on Drop, and should be removed
    // from SocketSet once the close handshake completes.
    //
    // Lock order invariant: SocketSet -> PENDING_TCP_CLOSE (matches Drop path).
    static ref PENDING_TCP_CLOSE: SpinLock<alloc::vec::Vec<SmoltcpHandle>> =
        SpinLock::new(alloc::vec::Vec::new());
}


use crate::uapi::fcntl::OpenFlags;
use crate::uapi::socket::SocketOptions;

pub struct SocketFile {
    handle: SpinLock<Option<SocketHandle>>,
    listen_sockets: SpinLock<alloc::vec::Vec<SocketHandle>>,
    listen_backlog: SpinLock<usize>,
    local_endpoint: SpinLock<Option<IpEndpoint>>,
    remote_endpoint: SpinLock<Option<IpEndpoint>>,
    shutdown_rd: SpinLock<bool>,
    shutdown_wr: SpinLock<bool>,
    flags: SpinLock<OpenFlags>,
    options: SpinLock<SocketOptions>,
    is_listener: SpinLock<bool>,
}

impl SocketFile {
    pub fn new(handle: SocketHandle) -> Self {
        Self {
            handle: SpinLock::new(Some(handle)),
            listen_sockets: SpinLock::new(alloc::vec::Vec::new()),
            listen_backlog: SpinLock::new(0),
            local_endpoint: SpinLock::new(None),
            remote_endpoint: SpinLock::new(None),
            shutdown_rd: SpinLock::new(false),
            shutdown_wr: SpinLock::new(false),
            flags: SpinLock::new(OpenFlags::empty()),
            options: SpinLock::new(SocketOptions::default()),
            is_listener: SpinLock::new(false),
        }
    }

    pub fn set_listener(&self, is_listener: bool) {
        *self.is_listener.lock() = is_listener;
    }

    pub fn is_listener(&self) -> bool {
        *self.is_listener.lock()
    }

    pub fn new_with_flags(handle: SocketHandle, flags: OpenFlags) -> Self {
        Self {
            handle: SpinLock::new(Some(handle)),
            listen_sockets: SpinLock::new(alloc::vec::Vec::new()),
            listen_backlog: SpinLock::new(0),
            local_endpoint: SpinLock::new(None),
            remote_endpoint: SpinLock::new(None),
            shutdown_rd: SpinLock::new(false),
            shutdown_wr: SpinLock::new(false),
            flags: SpinLock::new(flags),
            options: SpinLock::new(SocketOptions::default()),
            is_listener: SpinLock::new(false),
        }
    }

    pub fn set_listen_backlog(&self, backlog: usize) {
        *self.listen_backlog.lock() = backlog;
    }

    pub fn listen_backlog(&self) -> usize {
        *self.listen_backlog.lock()
    }

    pub fn get_socket_options(&self) -> SocketOptions {
        *self.options.lock()
    }

    pub fn set_socket_options(&self, opts: SocketOptions) {
        *self.options.lock() = opts;
    }

    pub fn handle(&self) -> SocketHandle {
        self.handle.lock().expect("SocketFile has no handle")
    }

    pub fn add_listen_socket(&self, handle: SocketHandle) {
        self.listen_sockets.lock().push(handle);
    }

    pub fn get_listen_sockets(&self) -> alloc::vec::Vec<SocketHandle> {
        self.listen_sockets.lock().clone()
    }

    pub fn clear_listen_sockets(&self) {
        self.listen_sockets.lock().clear();
    }

    pub fn listen_sockets_len(&self) -> usize {
        self.listen_sockets.lock().len()
    }

    /// Pop one established connection from the listener queue.
    ///
    /// This is used to provide a minimal accept/backlog behavior on top of smoltcp's
    /// single-socket listen model.
    pub fn take_established_from_listen_queue(&self) -> Option<SocketHandle> {
        let sockets = SOCKET_SET.lock();
        let mut q = self.listen_sockets.lock();
        let mut i = 0;
        while i < q.len() {
            match q[i] {
                SocketHandle::Tcp(h) => {
                    let s = sockets.get::<tcp::Socket>(h);
                    match s.state() {
                        tcp::State::Established | tcp::State::CloseWait => return Some(q.remove(i)),
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

    pub fn set_handle(&self, new_handle: SocketHandle) {
        *self.handle.lock() = Some(new_handle);
    }

    pub fn set_local_endpoint(&self, endpoint: IpEndpoint) {
        *self.local_endpoint.lock() = Some(endpoint);
    }

    pub fn get_local_endpoint(&self) -> Option<IpEndpoint> {
        *self.local_endpoint.lock()
    }

    pub fn set_remote_endpoint(&self, endpoint: IpEndpoint) {
        *self.remote_endpoint.lock() = Some(endpoint);
    }

    pub fn get_remote_endpoint(&self) -> Option<IpEndpoint> {
        *self.remote_endpoint.lock()
    }

    pub fn shutdown_read(&self) {
        *self.shutdown_rd.lock() = true;
    }

    pub fn shutdown_write(&self) {
        *self.shutdown_wr.lock() = true;
    }

    pub fn is_shutdown_read(&self) -> bool {
        *self.shutdown_rd.lock()
    }

    pub fn is_shutdown_write(&self) -> bool {
        *self.shutdown_wr.lock()
    }

    pub fn is_closed(&self) -> bool {
        let sockets = SOCKET_SET.lock();
        match self.handle.lock().as_ref() {
            Some(SocketHandle::Tcp(h)) => {
                let socket = sockets.get::<tcp::Socket>(*h);
                socket.state() == tcp::State::Closed
            }
            _ => false,
        }
    }
}

impl Drop for SocketFile {
    fn drop(&mut self) {
        let mut sockets = SOCKET_SET.lock();
        if let Some(handle) = *self.handle.lock() {
            match handle {
                SocketHandle::Tcp(h) => {
                    let socket = sockets.get_mut::<tcp::Socket>(h);
                    let state = socket.state();
                    crate::pr_debug!("[Socket] Drop: handle={:?}, state={:?}", h, state);
                    // Check if we need to close the socket.
                    //
                    // For active connections, do NOT remove from SocketSet immediately after close(),
                    // otherwise the peer may observe an abortive close and user programs (iperf3)
                    // can treat it as "unexpectedly closed".
                    match state {
                        tcp::State::Closed | tcp::State::TimeWait => {
                            // Fully closed, safe to remove now.
                            sockets.remove(h);
                        }
                        _ => {
                            // Initiate/continue graceful close, and defer removal until the stack
                            // transitions to Closed/TimeWait (requires polling).
                            crate::pr_debug!("[Socket] Drop: closing socket handle={:?}", h);
                            socket.close();
                            PENDING_TCP_CLOSE.lock().push(h);
                        }
                    }
                },
                SocketHandle::Udp(h) => { sockets.remove(h); },
            }
        }
        for handle in self.listen_sockets.lock().iter() {
            match handle {
                SocketHandle::Tcp(h) => { sockets.remove(*h); },
                SocketHandle::Udp(h) => { sockets.remove(*h); },
            }
        }
    }
}

/// Register a socket fd mapping (tid, fd) -> handle
pub fn register_socket_fd(tid: usize, fd: usize, handle: SocketHandle) {
    FD_SOCKET_MAP.lock().insert((tid, fd), handle);
}

/// Unregister a socket fd mapping
pub fn unregister_socket_fd(tid: usize, fd: usize) {
    FD_SOCKET_MAP.lock().remove(&(tid, fd));
}

/// Get socket handle from (tid, fd)
pub fn get_socket_handle(tid: usize, fd: usize) -> Option<SocketHandle> {
    FD_SOCKET_MAP.lock().get(&(tid, fd)).copied()
}

/// Update socket handle for an existing fd (used in accept)
pub fn update_socket_handle(tid: usize, fd: usize, handle: SocketHandle) {
    FD_SOCKET_MAP.lock().insert((tid, fd), handle);
}

/// Set local endpoint for a socket
pub fn set_socket_local_endpoint(
    file: &Arc<dyn crate::vfs::File>,
    endpoint: IpEndpoint,
) -> Result<(), ()> {
    let any = file.as_any();
    if let Some(socket_file) = any.downcast_ref::<SocketFile>() {
        socket_file.set_local_endpoint(endpoint);
        Ok(())
    } else {
        Err(())
    }
}

/// Get local endpoint from a socket
pub fn get_socket_local_endpoint(file: &Arc<dyn crate::vfs::File>) -> Option<IpEndpoint> {
    let any = file.as_any();
    any.downcast_ref::<SocketFile>()
        .and_then(|socket_file| socket_file.get_local_endpoint())
}

/// Set remote endpoint for a socket
pub fn set_socket_remote_endpoint(
    file: &Arc<dyn crate::vfs::File>,
    endpoint: IpEndpoint,
) -> Result<(), ()> {
    let any = file.as_any();
    if let Some(socket_file) = any.downcast_ref::<SocketFile>() {
        socket_file.set_remote_endpoint(endpoint);
        Ok(())
    } else {
        Err(())
    }
}

/// Get remote endpoint from a socket
pub fn get_socket_remote_endpoint(file: &Arc<dyn crate::vfs::File>) -> Option<IpEndpoint> {
    let any = file.as_any();
    any.downcast_ref::<SocketFile>()
        .and_then(|socket_file| socket_file.get_remote_endpoint())
}

/// Shutdown socket read
pub fn socket_shutdown_read(file: &Arc<dyn crate::vfs::File>) {
    if let Some(socket_file) = file.as_any().downcast_ref::<SocketFile>() {
        socket_file.shutdown_read();
    }
}

/// Shutdown socket write
pub fn socket_shutdown_write(file: &Arc<dyn crate::vfs::File>) {
    if let Some(socket_file) = file.as_any().downcast_ref::<SocketFile>() {
        socket_file.shutdown_write();
    }
}

/// Update socket handle in SocketFile (used in accept)
pub fn update_socket_file_handle(
    file: &Arc<dyn crate::vfs::File>,
    new_handle: SocketHandle,
) -> Result<(), ()> {
    let any = file.as_any();
    if let Some(socket_file) = any.downcast_ref::<SocketFile>() {
        socket_file.set_handle(new_handle);
        Ok(())
    } else {
        Err(())
    }
}

/// Send data to a specific endpoint (for sendto syscall)
pub fn socket_sendto(
    handle: SocketHandle,
    buf: &[u8],
    endpoint: IpEndpoint,
) -> Result<usize, FsError> {
    let result = {
        let mut sockets = SOCKET_SET.lock();
        match handle {
            SocketHandle::Tcp(_) => Err(FsError::NotSupported), // TCP doesn't support sendto
            SocketHandle::Udp(h) => {
                let socket = sockets.get_mut::<udp::Socket>(h);
                socket
                    .send_slice(buf, endpoint)
                    .map_err(|_| FsError::WouldBlock)?;
                Ok(buf.len())
            }
        }
    };
    if result.is_ok() {
        poll_network_interfaces();
    }
    result
}

impl File for SocketFile {
    fn readable(&self) -> bool {
        // Listener socket: only readable when a connection is ready to accept.
        if *self.is_listener.lock() {
            let sockets = SOCKET_SET.lock();

            for handle in self.listen_sockets.lock().iter() {
                if let SocketHandle::Tcp(h) = handle {
                    let s = sockets.get::<tcp::Socket>(*h);
                    if matches!(s.state(), tcp::State::Established | tcp::State::CloseWait) {
                        return true;
                    }
                }
            }

            if let Some(SocketHandle::Tcp(h)) = *self.handle.lock() {
                let s = sockets.get::<tcp::Socket>(h);
                return matches!(s.state(), tcp::State::Established | tcp::State::CloseWait);
            }
            return false;
        }

        let sockets = SOCKET_SET.lock();
        match self.handle.lock().as_ref() {
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
                // Closed sockets are readable (EOF)
                can_recv || state == tcp::State::Closed
            }
            Some(SocketHandle::Udp(h)) => {
                let socket = sockets.get::<udp::Socket>(*h);
                socket.can_recv()
            }
            None => false,
        }
    }
    fn writable(&self) -> bool {
        let sockets = SOCKET_SET.lock();
        let result = match self.handle.lock().as_ref() {
            Some(SocketHandle::Tcp(h)) => {
                let socket = sockets.get::<tcp::Socket>(*h);
                let can_send = socket.can_send();
                let state = socket.state();
                crate::pr_debug!("[Socket] writable: handle={:?}, state={:?}, can_send={}", h, state, can_send);
                can_send
            }
            Some(SocketHandle::Udp(h)) => {
                let socket = sockets.get::<udp::Socket>(*h);
                socket.can_send()
            }
            None => false,
        };
        result
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        if self.is_shutdown_read() {
            return Ok(0); // EOF
        }

        let mut sockets = SOCKET_SET.lock();
        let result = match self.handle.lock().as_ref() {
            Some(SocketHandle::Tcp(h)) => {
                let socket = sockets.get_mut::<tcp::Socket>(*h);
                let state = socket.state();
                let recv_queue = socket.recv_queue();
                crate::pr_debug!("[Socket] read: handle={:?}, state={:?}, recv_queue={}, buf.len()={}",
                    h, state, recv_queue, buf.len());

                // Closed socket returns EOF (0 bytes)
                if socket.state() == tcp::State::Closed {
                    return Ok(0);
                }

                let result = socket.recv_slice(buf).map_err(|_| FsError::WouldBlock);

                // CRITICAL FIX: smoltcp's recv_slice() returns Ok(0) when no data is available
                // but the socket is still connected. We need to distinguish between:
                // 1. No data available (should return EAGAIN for non-blocking, or block for blocking)
                // 2. Connection closed (should return 0 = EOF)
                if let Ok(0) = result {
                    // recv_slice returned 0 bytes - check if this is EOF or just no data
                    if socket.may_recv() {
                        // Socket can still receive data, so this is not EOF
                        // Return EAGAIN to indicate no data available
                        crate::pr_debug!("[Socket] read: recv_slice returned 0 but may_recv=true, returning EAGAIN");
                        Err(FsError::WouldBlock)
                    } else {
                        // Socket cannot receive anymore, this is EOF
                        crate::pr_debug!("[Socket] read: recv_slice returned 0 and may_recv=false, returning EOF");
                        Ok(0)
                    }
                } else {
                    if let Ok(n) = result {
                        crate::pr_debug!("[Socket] read: received {} bytes", n);
                    }
                    result
                }
            }
            Some(SocketHandle::Udp(h)) => {
                let socket = sockets.get_mut::<udp::Socket>(*h);
                socket
                    .recv_slice(buf)
                    .map(|(n, _)| n)
                    .map_err(|_| FsError::WouldBlock)
            }
            None => Err(FsError::InvalidArgument),
        };
        if result.is_ok() {
            crate::kernel::syscall::io::wake_poll_waiters();
        }
        result
    }

    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        if self.is_shutdown_write() {
            return Err(FsError::BrokenPipe);
        }

        let result = {
            let mut sockets = SOCKET_SET.lock();
            match self.handle.lock().as_ref() {
                Some(SocketHandle::Tcp(h)) => {
                    let socket = sockets.get_mut::<tcp::Socket>(*h);
                    let result = socket.send_slice(buf).map_err(|_| FsError::WouldBlock);

                    // Similar to recv_slice(), smoltcp may return Ok(0) when it cannot currently
                    // accept more data, even though the connection is still alive.
                    if !buf.is_empty() {
                        if let Ok(0) = result {
                            if socket.may_send() {
                                return Err(FsError::WouldBlock);
                            } else {
                                return Err(FsError::BrokenPipe);
                            }
                        }
                    }

                    result
                }
                Some(SocketHandle::Udp(h)) => {
                    let endpoint = match self.get_remote_endpoint() {
                        Some(ep) => ep,
                        None => return Err(FsError::NotConnected),
                    };
                    let socket = sockets.get_mut::<udp::Socket>(*h);
                    socket
                        .send_slice(buf, endpoint)
                        .map_err(|_| FsError::WouldBlock)?;
                    Ok(buf.len())
                }
                None => Err(FsError::InvalidArgument),
            }
        };
        if result.is_ok() {
            poll_network_interfaces();
            crate::kernel::syscall::io::wake_poll_waiters();
        }
        result
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        Err(FsError::NotSupported)
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn flags(&self) -> OpenFlags {
        *self.flags.lock()
    }

    fn set_status_flags(&self, new_flags: OpenFlags) -> Result<(), FsError> {
        *self.flags.lock() = new_flags;
        Ok(())
    }

    fn recvfrom(&self, buf: &mut [u8]) -> Result<(usize, Option<alloc::vec::Vec<u8>>), FsError> {
        if self.is_shutdown_read() {
            return Ok((0, None));
        }

        let mut sockets = SOCKET_SET.lock();
        match self.handle.lock().as_ref() {
            Some(SocketHandle::Tcp(h)) => {
                let socket = sockets.get_mut::<tcp::Socket>(*h);
                // Closed socket returns EOF (0 bytes)
                if socket.state() == tcp::State::Closed {
                    return Ok((0, None));
                }
                let n = socket.recv_slice(buf).map_err(|_| FsError::WouldBlock)?;
                let remote = socket.remote_endpoint().map(|ep| {
                    let mut addr_buf = alloc::vec![0u8; 16];
                    let _ = write_sockaddr_in_to_buf(&mut addr_buf, ep);
                    addr_buf
                });
                Ok((n, remote))
            }
            Some(SocketHandle::Udp(h)) => {
                let socket = sockets.get_mut::<udp::Socket>(*h);
                let (n, metadata) = socket.recv_slice(buf).map_err(|_| FsError::WouldBlock)?;
                let mut addr_buf = alloc::vec![0u8; 16];
                let _ = write_sockaddr_in_to_buf(&mut addr_buf, metadata.endpoint);
                Ok((n, Some(addr_buf)))
            }
            None => Err(FsError::InvalidArgument),
        }
    }
}

pub fn create_tcp_socket() -> Result<SocketHandle, ()> {
    let mut rx_vec = alloc::vec::Vec::new();
    rx_vec.try_reserve(4096).map_err(|_| ())?;
    rx_vec.resize(4096, 0);

    let mut tx_vec = alloc::vec::Vec::new();
    tx_vec.try_reserve(4096).map_err(|_| ())?;
    tx_vec.resize(4096, 0);

    let rx_buffer = tcp::SocketBuffer::new(rx_vec);
    let tx_buffer = tcp::SocketBuffer::new(tx_vec);
    let socket = tcp::Socket::new(rx_buffer, tx_buffer);

    let handle = SOCKET_SET.lock().add(socket);
    Ok(SocketHandle::Tcp(handle))
}

pub fn create_udp_socket() -> Result<SocketHandle, ()> {
    // Allocate metadata buffers
    let mut rx_meta_vec = alloc::vec::Vec::new();
    rx_meta_vec.try_reserve(4).map_err(|_| ())?;
    rx_meta_vec.resize(4, udp::PacketMetadata::EMPTY);

    let mut tx_meta_vec = alloc::vec::Vec::new();
    tx_meta_vec.try_reserve(4).map_err(|_| ())?;
    tx_meta_vec.resize(4, udp::PacketMetadata::EMPTY);

    // Allocate data buffers
    let mut rx_data_vec = alloc::vec::Vec::new();
    rx_data_vec.try_reserve(4096).map_err(|_| ())?;
    rx_data_vec.resize(4096, 0);

    let mut tx_data_vec = alloc::vec::Vec::new();
    tx_data_vec.try_reserve(4096).map_err(|_| ())?;
    tx_data_vec.resize(4096, 0);

    let rx_buffer = udp::PacketBuffer::new(rx_meta_vec, rx_data_vec);
    let tx_buffer = udp::PacketBuffer::new(tx_meta_vec, tx_data_vec);
    let socket = udp::Socket::new(rx_buffer, tx_buffer);
    let handle = SOCKET_SET.lock().add(socket);
    Ok(SocketHandle::Udp(handle))
}

const AF_INET: u16 = 2;
const SOCKADDR_IN_SIZE: usize = 16;

/// Parse sockaddr_in structure
pub fn parse_sockaddr_in(addr: *const u8, addrlen: u32) -> Result<IpEndpoint, ()> {
    if (addrlen as usize) < SOCKADDR_IN_SIZE {
        return Err(());
    }

    unsafe {
        let family = core::ptr::read_unaligned(addr as *const u16);
        if family != AF_INET {
            return Err(());
        }

        let port = u16::from_be(core::ptr::read_unaligned(addr.add(2) as *const u16));
        let ip_bytes = core::slice::from_raw_parts(addr.add(4), 4);
        let ip = Ipv4Address::from_octets([ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]]);

        // Note: Loopback addresses (127.0.0.1) are handled by smoltcp internally
        // No need to map to external IP

        Ok(IpEndpoint::new(IpAddress::Ipv4(ip), port))
    }
}

/// Write sockaddr_in to buffer
fn write_sockaddr_in_to_buf(buf: &mut [u8], endpoint: IpEndpoint) -> Result<(), ()> {
    if buf.len() < SOCKADDR_IN_SIZE {
        return Err(());
    }

    // family
    buf[0..2].copy_from_slice(&AF_INET.to_ne_bytes());

    // port
    buf[2..4].copy_from_slice(&endpoint.port.to_be_bytes());

    // ip
    match endpoint.addr {
        IpAddress::Ipv4(ipv4) => {
            buf[4..8].copy_from_slice(&ipv4.octets());
        }
        #[cfg(feature = "proto-ipv6")]
        IpAddress::Ipv6(_) => {
            return Err(()); // IPv6 not supported in AF_INET
        }
        _ => {
            return Err(()); // Unknown address type
        }
    }

    // zero padding
    buf[8..16].fill(0);

    Ok(())
}

/// Write sockaddr_in structure
pub fn write_sockaddr_in(addr: *mut u8, addrlen: *mut u32, endpoint: IpEndpoint) -> Result<(), ()> {
    if addr.is_null() || addrlen.is_null() {
        return Ok(());
    }

    unsafe {
        let len = *addrlen as usize;
        if len < SOCKADDR_IN_SIZE {
            return Err(());
        }

        let buf = core::slice::from_raw_parts_mut(addr, SOCKADDR_IN_SIZE);
        write_sockaddr_in_to_buf(buf, endpoint)?;

        *addrlen = SOCKADDR_IN_SIZE as u32;
    }

    Ok(())
}

/// Initialize network interface (should be called during network setup)
pub fn init_network(mut smoltcp_iface: crate::net::interface::SmoltcpInterface) {
    let wrapper = NetIfaceWrapper {
        device: SpinLock::new(smoltcp_iface.device_adapter_mut().clone()),
        interface: SpinLock::new(smoltcp_iface.into_interface()),
    };
    *NET_IFACE.lock() = Some(wrapper);
}


/// Perform TCP connect with Context
pub fn tcp_connect(handle: SmoltcpHandle, remote: IpEndpoint, local: IpEndpoint) -> Result<(), ()> {
    crate::pr_debug!("tcp_connect: start, handle={:?}", handle);

    let iface_guard = NET_IFACE.lock();
    crate::pr_debug!("tcp_connect: got NET_IFACE lock");

    let wrapper = iface_guard.as_ref().ok_or(())?;

    let result = wrapper.with_context(|context| {
        crate::pr_debug!("tcp_connect: in with_context");
        let mut sockets = SOCKET_SET.lock();
        crate::pr_debug!("tcp_connect: got SOCKET_SET lock");
        let socket = sockets.get_mut::<tcp::Socket>(handle);
        crate::pr_debug!("tcp_connect: calling socket.connect");
        let r = socket.connect(context, remote, local).map_err(|e| {
            crate::pr_debug!("tcp_connect error: {:?}", e);
            ()
        });
        crate::pr_debug!("tcp_connect: socket.connect returned {:?}", r);
        r
    });

    // Poll immediately after connect to trigger SYN packet
    if result.is_ok() {
        crate::pr_debug!("tcp_connect: polling to send SYN");
        wrapper.poll(&SOCKET_SET);
    }

    drop(iface_guard);
    crate::pr_debug!("tcp_connect: done, result={:?}", result);
    result
}

/// Poll network interfaces to process packets
pub fn poll_network_interfaces() {
    if let Some(ref wrapper) = *NET_IFACE.lock() {
        crate::pr_debug!("poll_network_interfaces: calling poll");
        wrapper.poll(&SOCKET_SET);
    }
}

/// Poll until loopback queue is empty
pub fn poll_until_empty() {
    if let Some(ref wrapper) = *NET_IFACE.lock() {
        // Always poll at least once to process socket state changes
        wrapper.poll(&SOCKET_SET);

        // Then drain loopback queue if it has packets
        while wrapper.loopback_queue_len() > 0 {
            wrapper.poll(&SOCKET_SET);
        }
    }
}
