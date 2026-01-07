//! Socket options and constants

// Socket levels
pub const SOL_SOCKET: i32 = 1;
pub const IPPROTO_TCP: i32 = 6;
pub const IPPROTO_IP: i32 = 0;
pub const IPPROTO_IPV6: i32 = 41;

// Socket types and flags
pub const SOCK_STREAM: i32 = 1;
pub const SOCK_DGRAM: i32 = 2;
pub const SOCK_NONBLOCK: i32 = 0x800;
pub const SOCK_CLOEXEC: i32 = 0x80000;
pub const SOCK_TYPE_MASK: i32 = 0x0f;

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
pub const TCP_INFO: i32 = 11;
pub const TCP_CONGESTION: i32 = 13;

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

/// Linux `struct tcp_info` (subset used by tools like iperf3).
///
/// This is a compatibility struct for `getsockopt(IPPROTO_TCP, TCP_INFO, ...)`.
/// We currently fill it with best-effort placeholder values (smoltcp doesn't expose all metrics).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TcpInfo {
    pub tcpi_state: u8,
    pub tcpi_ca_state: u8,
    pub tcpi_retransmits: u8,
    pub tcpi_probes: u8,
    pub tcpi_backoff: u8,
    pub tcpi_options: u8,
    pub tcpi_snd_rcv_wscale: u8, // snd_wscale:4, rcv_wscale:4
    pub tcpi_rate_flags: u8,     // delivery_rate_app_limited:1, fastopen_client_fail:2, ...

    pub tcpi_rto: u32,
    pub tcpi_ato: u32,
    pub tcpi_snd_mss: u32,
    pub tcpi_rcv_mss: u32,

    pub tcpi_unacked: u32,
    pub tcpi_sacked: u32,
    pub tcpi_lost: u32,
    pub tcpi_retrans: u32,
    pub tcpi_fackets: u32,

    pub tcpi_last_data_sent: u32,
    pub tcpi_last_ack_sent: u32,
    pub tcpi_last_data_recv: u32,
    pub tcpi_last_ack_recv: u32,

    pub tcpi_pmtu: u32,
    pub tcpi_rcv_ssthresh: u32,
    pub tcpi_rtt: u32,
    pub tcpi_rttvar: u32,
    pub tcpi_snd_ssthresh: u32,
    pub tcpi_snd_cwnd: u32,
    pub tcpi_advmss: u32,
    pub tcpi_reordering: u32,

    pub tcpi_rcv_rtt: u32,
    pub tcpi_rcv_space: u32,

    pub tcpi_total_retrans: u32,

    pub tcpi_pacing_rate: u64,
    pub tcpi_max_pacing_rate: u64,
    pub tcpi_bytes_acked: u64,
    pub tcpi_bytes_received: u64,
    pub tcpi_segs_out: u32,
    pub tcpi_segs_in: u32,

    pub tcpi_notsent_bytes: u32,
    pub tcpi_min_rtt: u32,
    pub tcpi_data_segs_in: u32,
    pub tcpi_data_segs_out: u32,

    pub tcpi_delivery_rate: u64,

    pub tcpi_busy_time: u64,
    pub tcpi_rwnd_limited: u64,
    pub tcpi_sndbuf_limited: u64,

    pub tcpi_delivered: u32,
    pub tcpi_delivered_ce: u32,

    pub tcpi_bytes_sent: u64,
    pub tcpi_bytes_retrans: u64,
    pub tcpi_dsack_dups: u32,
    pub tcpi_reord_seen: u32,

    pub tcpi_rcv_ooopack: u32,

    pub tcpi_snd_wnd: u32,
    pub tcpi_rcv_wnd: u32,

    pub tcpi_rehash: u32,

    pub tcpi_total_rto: u16,
    pub tcpi_total_rto_recoveries: u16,
    pub tcpi_total_rto_time: u32,
}

impl TcpInfo {
    pub fn dummy_established() -> Self {
        // tcp_states.h: TCP_ESTABLISHED = 1
        Self {
            tcpi_state: 1,
            tcpi_snd_mss: 1460,
            tcpi_rcv_mss: 1460,
            tcpi_snd_cwnd: 10,
            tcpi_rtt: 1_000,  // usec
            tcpi_rttvar: 500, // usec
            tcpi_pmtu: 1500,
            ..Default::default()
        }
    }
}
