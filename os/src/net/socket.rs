//! Socket implementation using smoltcp

use crate::sync::SpinLock;
use crate::vfs::{File, FsError, InodeMetadata};
use alloc::vec;
use lazy_static::lazy_static;
use smoltcp::iface::{Interface, SocketHandle as SmoltcpHandle, SocketSet};
use smoltcp::socket::{tcp, udp};
use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address};

#[derive(Clone, Copy)]
pub enum SocketHandle {
    Tcp(SmoltcpHandle),
    Udp(SmoltcpHandle),
}

use alloc::collections::BTreeMap;
use alloc::sync::Arc;

lazy_static! {
    pub static ref SOCKET_SET: SpinLock<SocketSet<'static>> = SpinLock::new(SocketSet::new(vec![]));
    /// Map from fd to socket handle for syscall operations
    pub static ref FD_SOCKET_MAP: SpinLock<BTreeMap<(usize, usize), SocketHandle>> = SpinLock::new(BTreeMap::new());
    /// Global network interface for socket operations
    pub static ref GLOBAL_INTERFACE: SpinLock<Option<Interface>> = SpinLock::new(None);
}

use crate::uapi::fcntl::OpenFlags;
use crate::uapi::socket::SocketOptions;

pub struct SocketFile {
    handle: SpinLock<SocketHandle>,
    local_endpoint: SpinLock<Option<IpEndpoint>>,
    remote_endpoint: SpinLock<Option<IpEndpoint>>,
    shutdown_rd: SpinLock<bool>,
    shutdown_wr: SpinLock<bool>,
    flags: SpinLock<OpenFlags>,
    options: SpinLock<SocketOptions>,
}

impl SocketFile {
    pub fn new(handle: SocketHandle) -> Self {
        Self {
            handle: SpinLock::new(handle),
            local_endpoint: SpinLock::new(None),
            remote_endpoint: SpinLock::new(None),
            shutdown_rd: SpinLock::new(false),
            shutdown_wr: SpinLock::new(false),
            flags: SpinLock::new(OpenFlags::empty()),
            options: SpinLock::new(SocketOptions::default()),
        }
    }

    pub fn new_with_flags(handle: SocketHandle, flags: OpenFlags) -> Self {
        Self {
            handle: SpinLock::new(handle),
            local_endpoint: SpinLock::new(None),
            remote_endpoint: SpinLock::new(None),
            shutdown_rd: SpinLock::new(false),
            shutdown_wr: SpinLock::new(false),
            flags: SpinLock::new(flags),
            options: SpinLock::new(SocketOptions::default()),
        }
    }

    pub fn get_socket_options(&self) -> SocketOptions {
        *self.options.lock()
    }

    pub fn set_socket_options(&self, opts: SocketOptions) {
        *self.options.lock() = opts;
    }

    pub fn handle(&self) -> SocketHandle {
        *self.handle.lock()
    }

    pub fn set_handle(&self, new_handle: SocketHandle) {
        *self.handle.lock() = new_handle;
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
}

impl Drop for SocketFile {
    fn drop(&mut self) {
        let mut sockets = SOCKET_SET.lock();
        match *self.handle.lock() {
            SocketHandle::Tcp(h) => {
                sockets.remove(h);
            }
            SocketHandle::Udp(h) => {
                sockets.remove(h);
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
}

impl File for SocketFile {
    fn readable(&self) -> bool {
        let sockets = SOCKET_SET.lock();
        match *self.handle.lock() {
            SocketHandle::Tcp(h) => {
                let socket = sockets.get::<tcp::Socket>(h);
                socket.can_recv()
            }
            SocketHandle::Udp(h) => {
                let socket = sockets.get::<udp::Socket>(h);
                socket.can_recv()
            }
        }
    }
    fn writable(&self) -> bool {
        let sockets = SOCKET_SET.lock();
        match *self.handle.lock() {
            SocketHandle::Tcp(h) => {
                let socket = sockets.get::<tcp::Socket>(h);
                socket.can_send()
            }
            SocketHandle::Udp(h) => {
                let socket = sockets.get::<udp::Socket>(h);
                socket.can_send()
            }
        }
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        if self.is_shutdown_read() {
            return Ok(0); // EOF
        }

        let mut sockets = SOCKET_SET.lock();
        let result = match *self.handle.lock() {
            SocketHandle::Tcp(h) => {
                let socket = sockets.get_mut::<tcp::Socket>(h);
                socket.recv_slice(buf).map_err(|_| FsError::WouldBlock)
            }
            SocketHandle::Udp(h) => {
                let socket = sockets.get_mut::<udp::Socket>(h);
                socket
                    .recv_slice(buf)
                    .map(|(n, _)| n)
                    .map_err(|_| FsError::WouldBlock)
            }
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

        let mut sockets = SOCKET_SET.lock();
        let result = match *self.handle.lock() {
            SocketHandle::Tcp(h) => {
                let socket = sockets.get_mut::<tcp::Socket>(h);
                socket.send_slice(buf).map_err(|_| FsError::WouldBlock)
            }
            SocketHandle::Udp(h) => {
                let endpoint = match self.get_remote_endpoint() {
                    Some(ep) => ep,
                    None => return Err(FsError::NotConnected),
                };
                let socket = sockets.get_mut::<udp::Socket>(h);
                socket
                    .send_slice(buf, endpoint)
                    .map_err(|_| FsError::WouldBlock)?;
                Ok(buf.len())
            }
        };
        if result.is_ok() {
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
        match *self.handle.lock() {
            SocketHandle::Tcp(h) => {
                let socket = sockets.get_mut::<tcp::Socket>(h);
                let n = socket.recv_slice(buf).map_err(|_| FsError::WouldBlock)?;
                let remote = socket.remote_endpoint().map(|ep| {
                    let mut addr_buf = alloc::vec![0u8; 16];
                    let _ = write_sockaddr_in_to_buf(&mut addr_buf, ep);
                    addr_buf
                });
                Ok((n, remote))
            }
            SocketHandle::Udp(h) => {
                let socket = sockets.get_mut::<udp::Socket>(h);
                let (n, metadata) = socket.recv_slice(buf).map_err(|_| FsError::WouldBlock)?;
                let mut addr_buf = alloc::vec![0u8; 16];
                let _ = write_sockaddr_in_to_buf(&mut addr_buf, metadata.endpoint);
                Ok((n, Some(addr_buf)))
            }
        }
    }
}

pub fn create_tcp_socket() -> Result<SocketHandle, ()> {
    // Allocate buffers with fallible allocation
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

/// Initialize global interface (should be called during network setup)
pub fn init_global_interface(iface: Interface) {
    *GLOBAL_INTERFACE.lock() = Some(iface);
}

/// Perform TCP connect with Context
pub fn tcp_connect(handle: SmoltcpHandle, remote: IpEndpoint, local: IpEndpoint) -> Result<(), ()> {
    let mut iface_guard = GLOBAL_INTERFACE.lock();
    let iface = iface_guard.as_mut().ok_or(())?;

    let mut sockets = SOCKET_SET.lock();
    let socket = sockets.get_mut::<tcp::Socket>(handle);

    socket
        .connect(iface.context(), remote, local)
        .map_err(|_| ())
}
