//! Minimal AF_UNIX socket implementation.
//!
//! This is an in-kernel IPC transport. It deliberately does not use smoltcp:
//! AF_UNIX addresses are local paths/abstract names, not IP endpoints.

use crate::{
    arch::Arch,
    sync::SpinLock,
    uapi::{
        errno::{EADDRINUSE, ECONNREFUSED, EINTR, EINVAL, EOPNOTSUPP},
        fcntl::OpenFlags,
        socket::{AF_UNIX, SOCK_DGRAM, SOCK_STREAM, SocketOptions},
        time::TimeSpec,
    },
    util::user_buffer::{read_from_user, write_to_user},
    vfs::{File, FileMode, FsError, InodeMetadata, InodeType, split_path, vfs_lookup},
};
use alloc::{
    collections::{BTreeMap, VecDeque},
    string::{String, ToString},
    sync::{Arc, Weak},
    vec,
    vec::Vec,
};

const SOCKADDR_UN_PATH_OFFSET: usize = 2;
const UNIX_PATH_MAX: usize = 108;
const STREAM_BUFFER_CAPACITY: usize = 64 * 1024;
const DGRAM_QUEUE_CAPACITY: usize = 128;
const DGRAM_MAX_SIZE: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnixSocketAddr {
    Path(String),
    Abstract(Vec<u8>),
}

impl UnixSocketAddr {
    fn path_bytes(&self) -> Vec<u8> {
        match self {
            Self::Path(path) => {
                let mut bytes = path.as_bytes().to_vec();
                bytes.push(0);
                bytes
            }
            Self::Abstract(name) => {
                let mut bytes = Vec::with_capacity(name.len() + 1);
                bytes.push(0);
                bytes.extend_from_slice(name);
                bytes
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnixSocketKind {
    Stream,
    Datagram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamSide {
    A,
    B,
}

#[derive(Debug)]
struct UnixStreamConnection {
    a_to_b: VecDeque<u8>,
    b_to_a: VecDeque<u8>,
    a_closed_read: bool,
    a_closed_write: bool,
    b_closed_read: bool,
    b_closed_write: bool,
}

impl UnixStreamConnection {
    fn new() -> Self {
        Self {
            a_to_b: VecDeque::new(),
            b_to_a: VecDeque::new(),
            a_closed_read: false,
            a_closed_write: false,
            b_closed_read: false,
            b_closed_write: false,
        }
    }

    fn inbound(&self, side: StreamSide) -> &VecDeque<u8> {
        match side {
            StreamSide::A => &self.b_to_a,
            StreamSide::B => &self.a_to_b,
        }
    }

    fn outbound(&self, side: StreamSide) -> &VecDeque<u8> {
        match side {
            StreamSide::A => &self.a_to_b,
            StreamSide::B => &self.b_to_a,
        }
    }

    fn outbound_mut(&mut self, side: StreamSide) -> &mut VecDeque<u8> {
        match side {
            StreamSide::A => &mut self.a_to_b,
            StreamSide::B => &mut self.b_to_a,
        }
    }

    fn local_read_closed(&self, side: StreamSide) -> bool {
        match side {
            StreamSide::A => self.a_closed_read,
            StreamSide::B => self.b_closed_read,
        }
    }

    fn local_write_closed(&self, side: StreamSide) -> bool {
        match side {
            StreamSide::A => self.a_closed_write,
            StreamSide::B => self.b_closed_write,
        }
    }

    fn peer_read_closed(&self, side: StreamSide) -> bool {
        match side {
            StreamSide::A => self.b_closed_read,
            StreamSide::B => self.a_closed_read,
        }
    }

    fn peer_write_closed(&self, side: StreamSide) -> bool {
        match side {
            StreamSide::A => self.b_closed_write,
            StreamSide::B => self.a_closed_write,
        }
    }

    fn close_read(&mut self, side: StreamSide) {
        match side {
            StreamSide::A => self.a_closed_read = true,
            StreamSide::B => self.b_closed_read = true,
        }
    }

    fn close_write(&mut self, side: StreamSide) {
        match side {
            StreamSide::A => self.a_closed_write = true,
            StreamSide::B => self.b_closed_write = true,
        }
    }
}

#[derive(Clone)]
struct UnixDatagram {
    data: Vec<u8>,
    source: Option<UnixSocketAddr>,
}

enum UnixSocketState {
    Unconnected,
    Listening {
        backlog: usize,
        pending: VecDeque<Arc<UnixSocketFile>>,
    },
    Connected {
        conn: Arc<SpinLock<UnixStreamConnection>>,
        side: StreamSide,
    },
}

pub struct UnixSocketFile {
    kind: UnixSocketKind,
    self_ref: Weak<UnixSocketFile>,
    flags: SpinLock<OpenFlags>,
    options: SpinLock<SocketOptions>,
    local_addr: SpinLock<Option<UnixSocketAddr>>,
    peer_addr: SpinLock<Option<UnixSocketAddr>>,
    registered_addr: SpinLock<Option<UnixSocketAddr>>,
    state: SpinLock<UnixSocketState>,
    dgram_queue: SpinLock<VecDeque<UnixDatagram>>,
    dgram_peer: SpinLock<Option<Weak<UnixSocketFile>>>,
    shutdown_read: SpinLock<bool>,
    shutdown_write: SpinLock<bool>,
}

lazy_static::lazy_static! {
    static ref UNIX_BINDINGS: SpinLock<BTreeMap<UnixSocketAddr, Weak<UnixSocketFile>>> =
        SpinLock::new(BTreeMap::new());
}

pub fn create_unix_socket(
    socket_type: i32,
    flags: OpenFlags,
) -> Result<Arc<UnixSocketFile>, isize> {
    let kind = match socket_type {
        SOCK_STREAM => UnixSocketKind::Stream,
        SOCK_DGRAM => UnixSocketKind::Datagram,
        _ => return Err(-(crate::uapi::errno::ESOCKTNOSUPPORT as isize)),
    };
    Ok(UnixSocketFile::new(kind, flags))
}

pub fn create_unix_socket_pair(
    socket_type: i32,
    flags: OpenFlags,
) -> Result<(Arc<UnixSocketFile>, Arc<UnixSocketFile>), isize> {
    match socket_type {
        SOCK_STREAM => {
            let conn = Arc::new(SpinLock::new(UnixStreamConnection::new()));
            let left = UnixSocketFile::new_connected_stream(flags, conn.clone(), StreamSide::A);
            let right = UnixSocketFile::new_connected_stream(flags, conn, StreamSide::B);
            Ok((left, right))
        }
        SOCK_DGRAM => {
            let left = UnixSocketFile::new(UnixSocketKind::Datagram, flags);
            let right = UnixSocketFile::new(UnixSocketKind::Datagram, flags);
            *left.dgram_peer.lock() = Some(Arc::downgrade(&right));
            *right.dgram_peer.lock() = Some(Arc::downgrade(&left));
            Ok((left, right))
        }
        _ => Err(-(crate::uapi::errno::ESOCKTNOSUPPORT as isize)),
    }
}

pub fn parse_sockaddr_un(addr: *const u8, addrlen: u32) -> Result<UnixSocketAddr, isize> {
    if addr.is_null() || (addrlen as usize) < SOCKADDR_UN_PATH_OFFSET {
        return Err(-(EINVAL as isize));
    }

    let copy_len = (addrlen as usize).min(SOCKADDR_UN_PATH_OFFSET + UNIX_PATH_MAX);
    let mut buf = [0u8; SOCKADDR_UN_PATH_OFFSET + UNIX_PATH_MAX];
    unsafe {
        crate::arch::ArchImpl::copy_from_user(
            crate::arch::address::UA::from_usize(addr as usize),
            buf.as_mut_ptr(),
            copy_len,
        )
    }
    .map_err(|_| -(crate::uapi::errno::EFAULT as isize))?;

    let family = u16::from_ne_bytes([buf[0], buf[1]]);
    if family != AF_UNIX as u16 {
        return Err(-(EINVAL as isize));
    }

    let path_len = copy_len.saturating_sub(SOCKADDR_UN_PATH_OFFSET);
    if path_len == 0 {
        return Err(-(EINVAL as isize));
    }

    let path_bytes = &buf[SOCKADDR_UN_PATH_OFFSET..SOCKADDR_UN_PATH_OFFSET + path_len];
    if path_bytes[0] == 0 {
        return Ok(UnixSocketAddr::Abstract(path_bytes[1..].to_vec()));
    }

    let nul = path_bytes
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(path_bytes.len());
    if nul == 0 {
        return Err(-(EINVAL as isize));
    }

    let path = core::str::from_utf8(&path_bytes[..nul])
        .map_err(|_| -(EINVAL as isize))?
        .to_string();
    Ok(UnixSocketAddr::Path(path))
}

pub fn write_sockaddr_un(
    addr: *mut u8,
    addrlen: *mut u32,
    endpoint: Option<UnixSocketAddr>,
) -> Result<(), isize> {
    if addr.is_null() || addrlen.is_null() {
        return Ok(());
    }

    let endpoint = match endpoint {
        Some(endpoint) => endpoint,
        None => {
            write_to_user(addrlen, SOCKADDR_UN_PATH_OFFSET as u32);
            return Ok(());
        }
    };

    let user_len = read_from_user(addrlen as *const u32) as usize;
    let mut buf = [0u8; SOCKADDR_UN_PATH_OFFSET + UNIX_PATH_MAX];
    buf[0..2].copy_from_slice(&(AF_UNIX as u16).to_ne_bytes());
    let path = endpoint.path_bytes();
    let path_copy_len = path.len().min(UNIX_PATH_MAX);
    buf[SOCKADDR_UN_PATH_OFFSET..SOCKADDR_UN_PATH_OFFSET + path_copy_len]
        .copy_from_slice(&path[..path_copy_len]);

    let total_len = SOCKADDR_UN_PATH_OFFSET + path_copy_len;
    let copy_len = user_len.min(total_len);
    unsafe {
        crate::arch::ArchImpl::copy_to_user(
            buf.as_ptr(),
            crate::arch::address::UA::from_usize(addr as usize),
            copy_len,
        )
    }
    .map_err(|_| -(crate::uapi::errno::EFAULT as isize))?;
    write_to_user(addrlen, total_len as u32);
    Ok(())
}

impl UnixSocketFile {
    fn new(kind: UnixSocketKind, flags: OpenFlags) -> Arc<Self> {
        Arc::new_cyclic(|self_ref| Self {
            kind,
            self_ref: self_ref.clone(),
            flags: SpinLock::new(flags),
            options: SpinLock::new(SocketOptions::default()),
            local_addr: SpinLock::new(None),
            peer_addr: SpinLock::new(None),
            registered_addr: SpinLock::new(None),
            state: SpinLock::new(UnixSocketState::Unconnected),
            dgram_queue: SpinLock::new(VecDeque::new()),
            dgram_peer: SpinLock::new(None),
            shutdown_read: SpinLock::new(false),
            shutdown_write: SpinLock::new(false),
        })
    }

    fn new_connected_stream(
        flags: OpenFlags,
        conn: Arc<SpinLock<UnixStreamConnection>>,
        side: StreamSide,
    ) -> Arc<Self> {
        let socket = Self::new(UnixSocketKind::Stream, flags);
        *socket.state.lock() = UnixSocketState::Connected { conn, side };
        socket
    }

    pub fn get_socket_options(&self) -> SocketOptions {
        *self.options.lock()
    }

    pub fn set_socket_options(&self, options: SocketOptions) {
        *self.options.lock() = options;
    }

    pub fn bind(&self, addr: UnixSocketAddr) -> isize {
        if self.local_addr.lock().is_some() {
            return -(EINVAL as isize);
        }

        let self_arc = match self.self_ref.upgrade() {
            Some(socket) => socket,
            None => return -(EINVAL as isize),
        };

        if let UnixSocketAddr::Path(path) = &addr
            && let Err(errno) = create_socket_node(path)
        {
            return errno;
        }

        let mut bindings = UNIX_BINDINGS.lock();
        if let Some(existing) = bindings.get(&addr).and_then(Weak::upgrade) {
            if !Arc::ptr_eq(&existing, &self_arc) {
                return -(EADDRINUSE as isize);
            }
        }

        bindings.insert(addr.clone(), Arc::downgrade(&self_arc));
        *self.local_addr.lock() = Some(addr.clone());
        *self.registered_addr.lock() = Some(addr);
        0
    }

    pub fn listen(&self, backlog: i32) -> isize {
        if self.kind != UnixSocketKind::Stream {
            return -(EOPNOTSUPP as isize);
        }
        if backlog < 0 {
            return -(EINVAL as isize);
        }

        let mut state = self.state.lock();
        match &*state {
            UnixSocketState::Unconnected => {
                *state = UnixSocketState::Listening {
                    backlog: (backlog as usize).clamp(1, 128),
                    pending: VecDeque::new(),
                };
                0
            }
            UnixSocketState::Listening { .. } => 0,
            UnixSocketState::Connected { .. } => -(EINVAL as isize),
        }
    }

    pub fn accept(&self) -> Result<Arc<UnixSocketFile>, isize> {
        let mut state = self.state.lock();
        match &mut *state {
            UnixSocketState::Listening { pending, .. } => pending
                .pop_front()
                .ok_or(-(crate::uapi::errno::EAGAIN as isize)),
            _ => Err(-(EINVAL as isize)),
        }
    }

    pub fn connect(&self, addr: UnixSocketAddr) -> isize {
        match self.kind {
            UnixSocketKind::Stream => self.connect_stream(addr),
            UnixSocketKind::Datagram => {
                *self.peer_addr.lock() = Some(addr);
                0
            }
        }
    }

    fn connect_stream(&self, addr: UnixSocketAddr) -> isize {
        let listener = match lookup_bound_socket(&addr) {
            Some(listener) => listener,
            None => return -(ECONNREFUSED as isize),
        };

        let mut listener_state = listener.state.lock();
        let (backlog, pending) = match &mut *listener_state {
            UnixSocketState::Listening { backlog, pending } => (*backlog, pending),
            _ => return -(ECONNREFUSED as isize),
        };

        if pending.len() >= backlog {
            return -(crate::uapi::errno::EAGAIN as isize);
        }

        let conn = Arc::new(SpinLock::new(UnixStreamConnection::new()));
        let server =
            UnixSocketFile::new_connected_stream(*self.flags.lock(), conn.clone(), StreamSide::B);

        let server_local = listener.local_addr.lock().clone();
        *server.local_addr.lock() = server_local.clone();
        *server.peer_addr.lock() = self.local_addr.lock().clone();

        {
            let mut state = self.state.lock();
            match &*state {
                UnixSocketState::Unconnected => {
                    *state = UnixSocketState::Connected {
                        conn,
                        side: StreamSide::A,
                    };
                }
                UnixSocketState::Connected { .. } => {
                    return -(crate::uapi::errno::EISCONN as isize);
                }
                UnixSocketState::Listening { .. } => return -(EINVAL as isize),
            }
        }
        *self.peer_addr.lock() = server_local.or(Some(addr));

        pending.push_back(server);
        crate::kernel::syscall::io::wake_poll_waiters();
        0
    }

    pub fn shutdown(&self, how: i32) -> isize {
        if !(0..=2).contains(&how) {
            return -(EINVAL as isize);
        }

        if how == 0 || how == 2 {
            *self.shutdown_read.lock() = true;
        }
        if how == 1 || how == 2 {
            *self.shutdown_write.lock() = true;
        }

        if let UnixSocketState::Connected { conn, side } = &*self.state.lock() {
            let mut conn = conn.lock();
            if how == 0 || how == 2 {
                conn.close_read(*side);
            }
            if how == 1 || how == 2 {
                conn.close_write(*side);
            }
        }
        crate::kernel::syscall::io::wake_poll_waiters();
        0
    }

    pub fn local_addr(&self) -> Option<UnixSocketAddr> {
        self.local_addr.lock().clone()
    }

    pub fn peer_addr(&self) -> Option<UnixSocketAddr> {
        self.peer_addr.lock().clone()
    }

    pub fn send_to(&self, buf: &[u8], addr: UnixSocketAddr) -> Result<usize, FsError> {
        match self.kind {
            UnixSocketKind::Stream => self.write(buf),
            UnixSocketKind::Datagram => self.send_datagram(buf, Some(addr)),
        }
    }

    fn send_datagram(&self, buf: &[u8], dest: Option<UnixSocketAddr>) -> Result<usize, FsError> {
        if *self.shutdown_write.lock() {
            return Err(FsError::BrokenPipe);
        }
        if buf.len() > DGRAM_MAX_SIZE {
            return Err(FsError::InvalidArgument);
        }

        if let Some(peer) = self.dgram_peer.lock().as_ref().and_then(Weak::upgrade) {
            return enqueue_datagram(&peer, buf, self.local_addr());
        }

        let dest = dest
            .or_else(|| self.peer_addr())
            .ok_or(FsError::DestinationAddressRequired)?;
        let peer = lookup_bound_socket(&dest).ok_or(FsError::NotConnected)?;
        enqueue_datagram(&peer, buf, self.local_addr())
    }
}

impl File for UnixSocketFile {
    fn readable(&self) -> bool {
        if *self.shutdown_read.lock() {
            return true;
        }

        match self.kind {
            UnixSocketKind::Stream => match &*self.state.lock() {
                UnixSocketState::Listening { pending, .. } => !pending.is_empty(),
                UnixSocketState::Connected { conn, side } => {
                    let conn = conn.lock();
                    !conn.inbound(*side).is_empty() || conn.peer_write_closed(*side)
                }
                UnixSocketState::Unconnected => false,
            },
            UnixSocketKind::Datagram => !self.dgram_queue.lock().is_empty(),
        }
    }

    fn writable(&self) -> bool {
        if *self.shutdown_write.lock() {
            return false;
        }

        match self.kind {
            UnixSocketKind::Stream => match &*self.state.lock() {
                UnixSocketState::Connected { conn, side } => {
                    let conn = conn.lock();
                    !conn.local_write_closed(*side)
                        && !conn.peer_read_closed(*side)
                        && conn.outbound(*side).len() < STREAM_BUFFER_CAPACITY
                }
                _ => false,
            },
            UnixSocketKind::Datagram => {
                self.dgram_peer.lock().is_some() || self.peer_addr.lock().is_some()
            }
        }
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        if *self.shutdown_read.lock() {
            return Ok(0);
        }

        match self.kind {
            UnixSocketKind::Stream => match &*self.state.lock() {
                UnixSocketState::Connected { conn, side } => {
                    let mut conn = conn.lock();
                    if conn.local_read_closed(*side) {
                        return Ok(0);
                    }
                    if conn.inbound(*side).is_empty() {
                        if conn.peer_write_closed(*side) {
                            return Ok(0);
                        }
                        return Err(FsError::WouldBlock);
                    }

                    let nread = buf.len().min(conn.inbound(*side).len());
                    let inbound = match side {
                        StreamSide::A => &mut conn.b_to_a,
                        StreamSide::B => &mut conn.a_to_b,
                    };
                    for byte in buf.iter_mut().take(nread) {
                        *byte = inbound.pop_front().unwrap();
                    }
                    crate::kernel::syscall::io::wake_poll_waiters();
                    Ok(nread)
                }
                _ => Err(FsError::NotConnected),
            },
            UnixSocketKind::Datagram => {
                let datagram = self
                    .dgram_queue
                    .lock()
                    .pop_front()
                    .ok_or(FsError::WouldBlock)?;
                let nread = buf.len().min(datagram.data.len());
                buf[..nread].copy_from_slice(&datagram.data[..nread]);
                crate::kernel::syscall::io::wake_poll_waiters();
                Ok(nread)
            }
        }
    }

    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        if *self.shutdown_write.lock() {
            return Err(FsError::BrokenPipe);
        }

        match self.kind {
            UnixSocketKind::Stream => match &*self.state.lock() {
                UnixSocketState::Connected { conn, side } => {
                    let mut conn = conn.lock();
                    if conn.local_write_closed(*side) {
                        return Err(FsError::BrokenPipe);
                    }
                    if conn.peer_read_closed(*side) {
                        return Err(FsError::BrokenPipe);
                    }

                    let available =
                        STREAM_BUFFER_CAPACITY.saturating_sub(conn.outbound(*side).len());
                    if available == 0 {
                        return Err(FsError::WouldBlock);
                    }

                    let nwrite = buf.len().min(available);
                    let outbound = conn.outbound_mut(*side);
                    for &byte in &buf[..nwrite] {
                        outbound.push_back(byte);
                    }
                    crate::kernel::syscall::io::wake_poll_waiters();
                    Ok(nwrite)
                }
                _ => Err(FsError::NotConnected),
            },
            UnixSocketKind::Datagram => self.send_datagram(buf, None),
        }
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        Ok(InodeMetadata {
            inode_no: 0,
            inode_type: InodeType::Socket,
            size: 0,
            mode: FileMode::S_IFSOCK | FileMode::from_bits_truncate(0o777),
            uid: 0,
            gid: 0,
            atime: TimeSpec::zero(),
            mtime: TimeSpec::zero(),
            ctime: TimeSpec::zero(),
            nlinks: 1,
            blocks: 0,
            rdev: 0,
        })
    }

    fn flags(&self) -> OpenFlags {
        *self.flags.lock()
    }

    fn set_status_flags(&self, new_flags: OpenFlags) -> Result<(), FsError> {
        *self.flags.lock() = new_flags;
        Ok(())
    }

    fn recvfrom(&self, buf: &mut [u8]) -> Result<(usize, Option<Vec<u8>>), FsError> {
        match self.kind {
            UnixSocketKind::Stream => {
                let nread = self.read(buf)?;
                Ok((nread, self.peer_addr().map(sockaddr_bytes)))
            }
            UnixSocketKind::Datagram => {
                let datagram = self
                    .dgram_queue
                    .lock()
                    .pop_front()
                    .ok_or(FsError::WouldBlock)?;
                let nread = buf.len().min(datagram.data.len());
                buf[..nread].copy_from_slice(&datagram.data[..nread]);
                Ok((nread, datagram.source.map(sockaddr_bytes)))
            }
        }
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

impl Drop for UnixSocketFile {
    fn drop(&mut self) {
        if let Some(addr) = self.registered_addr.lock().take() {
            let self_ptr = self as *const UnixSocketFile;
            let mut bindings = UNIX_BINDINGS.lock();
            if let Some(bound) = bindings.get(&addr).and_then(Weak::upgrade)
                && Arc::as_ptr(&bound) == self_ptr
            {
                bindings.remove(&addr);
            }
        }

        if let UnixSocketState::Connected { conn, side } = &*self.state.lock() {
            let mut conn = conn.lock();
            conn.close_read(*side);
            conn.close_write(*side);
        }
        crate::kernel::syscall::io::wake_poll_waiters();
    }
}

fn lookup_bound_socket(addr: &UnixSocketAddr) -> Option<Arc<UnixSocketFile>> {
    let mut bindings = UNIX_BINDINGS.lock();
    match bindings.get(addr).and_then(Weak::upgrade) {
        Some(socket) => Some(socket),
        None => {
            bindings.remove(addr);
            None
        }
    }
}

fn enqueue_datagram(
    target: &Arc<UnixSocketFile>,
    buf: &[u8],
    source: Option<UnixSocketAddr>,
) -> Result<usize, FsError> {
    if *target.shutdown_read.lock() {
        return Err(FsError::NotConnected);
    }
    let mut queue = target.dgram_queue.lock();
    if queue.len() >= DGRAM_QUEUE_CAPACITY {
        return Err(FsError::WouldBlock);
    }
    queue.push_back(UnixDatagram {
        data: buf.to_vec(),
        source,
    });
    drop(queue);
    crate::kernel::syscall::io::wake_poll_waiters();
    Ok(buf.len())
}

fn sockaddr_bytes(addr: UnixSocketAddr) -> Vec<u8> {
    let path = addr.path_bytes();
    let total_len = SOCKADDR_UN_PATH_OFFSET + path.len().min(UNIX_PATH_MAX);
    let mut buf = vec![0u8; total_len];
    buf[0..2].copy_from_slice(&(AF_UNIX as u16).to_ne_bytes());
    buf[SOCKADDR_UN_PATH_OFFSET..].copy_from_slice(&path[..total_len - SOCKADDR_UN_PATH_OFFSET]);
    buf
}

fn create_socket_node(path: &str) -> Result<(), isize> {
    if path.is_empty() {
        return Err(-(EINVAL as isize));
    }

    if vfs_lookup(path).is_ok() {
        return Err(-(EADDRINUSE as isize));
    }

    let (dir_path, name) = split_path(path).map_err(|_| -(EINVAL as isize))?;
    let parent = vfs_lookup(&dir_path).map_err(|e| e.to_errno())?;
    let mode = FileMode::S_IFSOCK | FileMode::from_bits_truncate(0o777);
    parent
        .inode
        .mknod(&name, mode, 0)
        .map(|_| ())
        .map_err(|e| e.to_errno())
}

pub fn wait_unix_would_block(
    file: Arc<dyn File>,
    task: crate::kernel::SharedTask,
) -> Result<(), isize> {
    drop(file);
    crate::kernel::yield_task();
    if crate::ipc::signal_interrupts_syscall(&task) {
        return Err(-(EINTR as isize));
    }
    Ok(())
}

pub fn write_socketpair_fds(sv: *mut i32, first: usize, second: usize) -> Result<(), isize> {
    if sv.is_null() {
        return Err(-(EINVAL as isize));
    }
    write_to_user(sv, first as i32);
    unsafe {
        write_to_user(sv.add(1), second as i32);
    }
    Ok(())
}
