//! Socket options and constants

// Socket levels
pub const SOL_SOCKET: i32 = 1;
pub const IPPROTO_TCP: i32 = 6;
pub const IPPROTO_IP: i32 = 0;
pub const IPPROTO_IPV6: i32 = 41;

// SOL_SOCKET options
pub const SO_REUSEADDR: i32 = 2;
pub const SO_KEEPALIVE: i32 = 9;
pub const SO_SNDBUF: i32 = 7;
pub const SO_RCVBUF: i32 = 8;
pub const SO_LINGER: i32 = 13;
pub const SO_REUSEPORT: i32 = 15;
pub const SO_RCVTIMEO_OLD: i32 = 20;
pub const SO_SNDTIMEO_OLD: i32 = 21;

// IPPROTO_TCP options
pub const TCP_NODELAY: i32 = 1;
pub const TCP_MAXSEG: i32 = 2;

// IPPROTO_IPV6 options
pub const IPV6_V6ONLY: i32 = 26;

/// Socket options storage
#[derive(Clone, Copy, Debug)]
pub struct SocketOptions {
    pub reuse_addr: bool,
    pub reuse_port: bool,
    pub keepalive: bool,
    pub tcp_nodelay: bool,
    pub ipv6_v6only: bool,
    pub tcp_maxseg: usize,
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
            ipv6_v6only: true,
            tcp_maxseg: 1460, // Default MSS for IPv4
            send_buffer_size: 65536,
            recv_buffer_size: 65536,
        }
    }
}
