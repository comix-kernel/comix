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

lazy_static! {
    pub static ref SOCKET_SET: SpinLock<SocketSet<'static>> = SpinLock::new(SocketSet::new(vec![]));
}

pub struct SocketFile {
    handle: SocketHandle,
}

impl SocketFile {
    pub fn new(handle: SocketHandle) -> Self {
        Self { handle }
    }

    pub fn handle(&self) -> SocketHandle {
        self.handle
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
                let socket = sockets.get_mut::<udp::Socket>(h);
                let endpoint = IpEndpoint::new(IpAddress::Ipv4(Ipv4Address::UNSPECIFIED), 0);
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
    let rx_buffer = tcp::SocketBuffer::new(vec![0; 4096]);
    let tx_buffer = tcp::SocketBuffer::new(vec![0; 4096]);
    let socket = tcp::Socket::new(rx_buffer, tx_buffer);
    let handle = SOCKET_SET.lock().add(socket);
    Ok(SocketHandle::Tcp(handle))
}

pub fn create_udp_socket() -> Result<SocketHandle, ()> {
    let rx_meta = udp::PacketMetadata::EMPTY;
    let tx_meta = udp::PacketMetadata::EMPTY;
    let rx_buffer = udp::PacketBuffer::new(vec![rx_meta; 4], vec![0; 4096]);
    let tx_buffer = udp::PacketBuffer::new(vec![tx_meta; 4], vec![0; 4096]);
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
