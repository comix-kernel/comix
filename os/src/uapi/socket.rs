//! Socket options and constants

// Socket levels
pub const SOL_SOCKET: i32 = 1;
pub const IPPROTO_TCP: i32 = 6;
pub const IPPROTO_IP: i32 = 0;

// SOL_SOCKET options
pub const SO_REUSEADDR: i32 = 2;
pub const SO_KEEPALIVE: i32 = 9;
pub const SO_SNDBUF: i32 = 7;
pub const SO_RCVBUF: i32 = 8;
pub const SO_LINGER: i32 = 13;
pub const SO_REUSEPORT: i32 = 15;

// IPPROTO_TCP options
pub const TCP_NODELAY: i32 = 1;

/// Socket options storage
#[derive(Clone, Copy, Debug)]
pub struct SocketOptions {
    pub reuse_addr: bool,
    pub reuse_port: bool,
    pub keepalive: bool,
    pub tcp_nodelay: bool,
    pub send_buffer_size: usize,
    pub recv_buffer_size: usize,
}

impl Default for SocketOptions {
    fn default() -> Self {
        Self {
            reuse_addr: false,
            reuse_port: false,
            keepalive: false,
            tcp_nodelay: false,
            send_buffer_size: 65536, // 64KB default
            recv_buffer_size: 65536, // 64KB default
        }
    }
}
