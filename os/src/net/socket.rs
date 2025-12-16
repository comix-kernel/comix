//! Socket implementation using smoltcp

use crate::sync::SpinLock;
use crate::vfs::{File, FsError, InodeMetadata};
use alloc::vec;
use lazy_static::lazy_static;
use smoltcp::iface::{SocketHandle as SmoltcpHandle, SocketSet};
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
}

pub struct SocketFile {
    handle: SocketHandle,
    remote_endpoint: SpinLock<Option<IpEndpoint>>,
}

impl SocketFile {
    pub fn new(handle: SocketHandle) -> Self {
        Self {
            handle,
            remote_endpoint: SpinLock::new(None),
        }
    }

    pub fn handle(&self) -> SocketHandle {
        self.handle
    }

    pub fn set_remote_endpoint(&self, endpoint: IpEndpoint) {
        *self.remote_endpoint.lock() = Some(endpoint);
    }

    pub fn get_remote_endpoint(&self) -> Option<IpEndpoint> {
        *self.remote_endpoint.lock()
    }
}

impl Drop for SocketFile {
    fn drop(&mut self) {
        let mut sockets = SOCKET_SET.lock();
        match self.handle {
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

/// Set remote endpoint for a socket
pub fn set_socket_remote_endpoint(
    file: &Arc<dyn crate::vfs::File>,
    endpoint: IpEndpoint,
) -> Result<(), ()> {
    // SAFETY: We assume the file is a SocketFile if it's in FD_SOCKET_MAP
    let ptr = Arc::as_ptr(file) as *const SocketFile;
    unsafe {
        (*ptr).set_remote_endpoint(endpoint);
    }
    Ok(())
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
        true
    }
    fn writable(&self) -> bool {
        true
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        let mut sockets = SOCKET_SET.lock();
        match self.handle {
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
        }
    }

    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        let mut sockets = SOCKET_SET.lock();
        match self.handle {
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
        }
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        Err(FsError::NotSupported)
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

/// Parse sockaddr_in structure
pub fn parse_sockaddr_in(addr: *const u8, addrlen: u32) -> Result<IpEndpoint, ()> {
    if addrlen < 16 {
        return Err(());
    }

    unsafe {
        let family = *(addr as *const u16);
        if family != 2 {
            // AF_INET
            return Err(());
        }

        let port = u16::from_be(*(addr.add(2) as *const u16));
        let ip_bytes = core::slice::from_raw_parts(addr.add(4), 4);
        let ip = Ipv4Address::from_octets([ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]]);

        Ok(IpEndpoint::new(IpAddress::Ipv4(ip), port))
    }
}

/// Write sockaddr_in structure
pub fn write_sockaddr_in(addr: *mut u8, addrlen: *mut u32, endpoint: IpEndpoint) -> Result<(), ()> {
    if addr.is_null() || addrlen.is_null() {
        return Ok(());
    }

    unsafe {
        let len = *addrlen as usize;
        if len < 16 {
            return Err(());
        }

        // family
        *(addr as *mut u16) = 2; // AF_INET

        // port
        *(addr.add(2) as *mut u16) = endpoint.port.to_be();

        // ip
        if let IpAddress::Ipv4(ipv4) = endpoint.addr {
            let octets = ipv4.octets();
            core::ptr::copy_nonoverlapping(octets.as_ptr(), addr.add(4), 4);
        }

        // zero padding
        core::ptr::write_bytes(addr.add(8), 0, 8);

        *addrlen = 16;
    }

    Ok(())
}
