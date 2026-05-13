//! Socket implementation using smoltcp

use crate::hal::arch::Arch;
use crate::sync::SpinLock;
use crate::vfs::{File, FsError, InodeMetadata};
use alloc::collections::VecDeque;
use lazy_static::lazy_static;
use smoltcp::iface::SocketHandle as SmoltcpHandle;
use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address};

#[derive(Clone, Copy, Debug)]
pub enum SocketHandle {
    Tcp(SmoltcpHandle),
    Udp(SmoltcpHandle),
}

use alloc::collections::BTreeMap;
use alloc::sync::Arc;

lazy_static! {
    pub static ref FD_SOCKET_MAP: SpinLock<BTreeMap<(usize, usize), SocketHandle>> =
        SpinLock::new(BTreeMap::new());
}

use crate::uapi::fcntl::OpenFlags;
use crate::uapi::socket::SocketOptions;

const UDP_RXQ_CAP: usize = 64;
pub(crate) const UDP_DGRAM_MAX: usize = 2048;

#[derive(Debug)]
pub(crate) struct UdpDatagram {
    pub(crate) src: IpEndpoint,
    pub(crate) len: usize,
    pub(crate) data: [u8; UDP_DGRAM_MAX],
}

pub struct SocketFile {
    pub(crate) handle: SpinLock<Option<SocketHandle>>,
    pub(crate) listen_sockets: SpinLock<alloc::vec::Vec<SocketHandle>>,
    listen_backlog: SpinLock<usize>,
    local_endpoint: SpinLock<Option<IpEndpoint>>,
    remote_endpoint: SpinLock<Option<IpEndpoint>>,
    udp_rx_queue: SpinLock<VecDeque<UdpDatagram>>,
    shutdown_rd: SpinLock<bool>,
    shutdown_wr: SpinLock<bool>,
    flags: SpinLock<OpenFlags>,
    options: SpinLock<SocketOptions>,
    pub(crate) is_listener: SpinLock<bool>,
}

impl SocketFile {
    pub fn new(handle: SocketHandle) -> Self {
        Self {
            handle: SpinLock::new(Some(handle)),
            listen_sockets: SpinLock::new(alloc::vec::Vec::new()),
            listen_backlog: SpinLock::new(0),
            local_endpoint: SpinLock::new(None),
            remote_endpoint: SpinLock::new(None),
            udp_rx_queue: SpinLock::new(VecDeque::with_capacity(UDP_RXQ_CAP)),
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
            udp_rx_queue: SpinLock::new(VecDeque::with_capacity(UDP_RXQ_CAP)),
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
        crate::net::stack::network_stack().take_established_from_listen_queue(self)
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

    pub(crate) fn udp_queue_len(&self) -> usize {
        self.udp_rx_queue.lock().len()
    }

    pub(crate) fn udp_push(&self, d: UdpDatagram) -> bool {
        let mut q = self.udp_rx_queue.lock();
        if q.len() == q.capacity() {
            return false;
        }
        q.push_back(d);
        true
    }

    pub(crate) fn udp_pop(&self) -> Option<UdpDatagram> {
        self.udp_rx_queue.lock().pop_front()
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
        crate::net::stack::network_stack().socket_is_closed(self)
    }
}

impl Drop for SocketFile {
    fn drop(&mut self) {
        crate::net::stack::network_stack().drop_socket_file(self);
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
    crate::net::stack::network_stack().socket_sendto(handle, buf, endpoint)
}

impl File for SocketFile {
    fn readable(&self) -> bool {
        crate::net::stack::network_stack().socket_readable(self)
    }
    fn writable(&self) -> bool {
        crate::net::stack::network_stack().socket_writable(self)
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        crate::net::stack::network_stack().socket_read(self, buf)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        crate::net::stack::network_stack().socket_write(self, buf)
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
        crate::net::stack::network_stack().socket_recvfrom(self, buf)
    }
}

const AF_INET: u16 = 2;
const SOCKADDR_IN_SIZE: usize = 16;

/// Parse sockaddr_in structure from user space
pub fn parse_sockaddr_in(addr: *const u8, addrlen: u32) -> Result<IpEndpoint, ()> {
    if (addrlen as usize) < SOCKADDR_IN_SIZE {
        return Err(());
    }

    let mut buf = [0u8; SOCKADDR_IN_SIZE];
    unsafe { crate::arch::ArchImpl::copy_from_user(addr as usize, buf.as_mut_ptr(), SOCKADDR_IN_SIZE) }
        .map_err(|_| ())?;

    let family = u16::from_ne_bytes([buf[0], buf[1]]);
    if family != AF_INET {
        return Err(());
    }

    let port = u16::from_be_bytes([buf[2], buf[3]]);
    let ip = Ipv4Address::from_octets([buf[4], buf[5], buf[6], buf[7]]);

    // Note: Loopback addresses (127.0.0.1) are handled by smoltcp internally
    // No need to map to external IP

    Ok(IpEndpoint::new(IpAddress::Ipv4(ip), port))
}

/// Write sockaddr_in to buffer
pub(crate) fn write_sockaddr_in_to_buf(buf: &mut [u8], endpoint: IpEndpoint) -> Result<(), ()> {
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
        #[cfg(not(feature = "proto-ipv6"))]
        _ => {
            return Err(()); // Unknown address type
        }
    }

    // zero padding
    buf[8..16].fill(0);

    Ok(())
}

/// Write sockaddr_in structure to user space
pub fn write_sockaddr_in(addr: *mut u8, addrlen: *mut u32, endpoint: IpEndpoint) -> Result<(), ()> {
    if addr.is_null() || addrlen.is_null() {
        return Ok(());
    }

    use crate::util::user_buffer::read_from_user;

    let len = read_from_user(addrlen as *const u32) as usize;

    // Linux behavior: truncate the stored address if the provided buffer is too small.
    // Still report the full required length back to user space.
    crate::util::user_buffer::write_to_user(addrlen, SOCKADDR_IN_SIZE as u32);
    if len == 0 {
        return Ok(());
    }

    let mut tmp = [0u8; SOCKADDR_IN_SIZE];
    write_sockaddr_in_to_buf(&mut tmp, endpoint)?;
    let n = core::cmp::min(len, SOCKADDR_IN_SIZE);
    unsafe { crate::arch::ArchImpl::copy_to_user(tmp.as_ptr(), addr as usize, n) }.map_err(|_| ())?;

    Ok(())
}

pub fn create_tcp_socket() -> Result<SocketHandle, ()> {
    crate::net::stack::network_stack().create_tcp_socket()
}

pub fn create_udp_socket() -> Result<SocketHandle, ()> {
    crate::net::stack::network_stack().create_udp_socket()
}

/// Initialize network interface through the stack facade.
pub fn init_network(smoltcp_iface: crate::net::interface::SmoltcpInterface) {
    crate::net::stack::network_stack().init_network(smoltcp_iface);
}

pub fn tcp_connect(handle: SmoltcpHandle, remote: IpEndpoint, local: IpEndpoint) -> Result<(), ()> {
    crate::net::stack::network_stack().tcp_connect(handle, remote, local)
}

/// Poll network interfaces to process packets.
pub fn poll_network_interfaces() {
    crate::net::stack::network_stack().poll();
}

/// Poll smoltcp + dispatch UDP datagrams to per-fd queues.
pub fn poll_network_and_dispatch() {
    crate::net::stack::network_stack().poll_and_dispatch();
}

/// Drain UDP datagrams from shared per-port sockets and deliver them to per-fd queues.
pub fn udp_dispatch() -> bool {
    crate::net::stack::network_stack().udp_dispatch()
}

pub fn udp_attach_fd_to_port(
    tid: usize,
    fd: usize,
    file: &Arc<dyn crate::vfs::File>,
    old_handle: SmoltcpHandle,
    port: u16,
    bind_addr: Option<IpAddress>,
) -> Result<SmoltcpHandle, ()> {
    crate::net::stack::network_stack()
        .udp_attach_fd_to_port(tid, fd, file, old_handle, port, bind_addr)
}

/// Poll until loopback queue is empty.
pub fn poll_until_empty() {
    crate::net::stack::network_stack().poll_until_empty();
}
