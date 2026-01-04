//! Loopback network device implementation

use crate::device::net::net_device::{NetDevice, NetDeviceError};
use crate::sync::SpinLock;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;

/// Loopback device - pure software device for local communication
pub struct LoopbackDevice {
    queue: Arc<SpinLock<VecDeque<Vec<u8>>>>,
    mtu: usize,
}

impl LoopbackDevice {
    /// Create a new loopback device
    pub fn new() -> Self {
        Self {
            queue: Arc::new(SpinLock::new(VecDeque::with_capacity(64))),
            mtu: 65535,
        }
    }
}

impl NetDevice for LoopbackDevice {
    fn receive(&self, buf: &mut [u8]) -> Result<usize, NetDeviceError> {
        let mut queue = self.queue.lock();
        let queue_len = queue.len();
        if let Some(packet) = queue.pop_front() {
            let len = packet.len().min(buf.len());
            buf[..len].copy_from_slice(&packet[..len]);
            crate::pr_debug!("[Loopback] receive: got {} bytes from queue (queue had {} packets)", len, queue_len);
            Ok(len)
        } else {
            Err(NetDeviceError::QueueEmpty)
        }
    }

    fn send(&self, buf: &[u8]) -> Result<(), NetDeviceError> {
        let mut queue = self.queue.lock();
        if queue.len() < 64 {
            queue.push_back(buf.to_vec());
            crate::pr_debug!("[Loopback] send: queued {} bytes (queue now has {} packets)", buf.len(), queue.len());
            Ok(())
        } else {
            Err(NetDeviceError::QueueFull)
        }
    }

    fn device_id(&self) -> usize {
        0 // Loopback always has ID 0
    }

    fn mtu(&self) -> usize {
        self.mtu
    }

    fn name(&self) -> &str {
        "lo"
    }

    fn mac_address(&self) -> [u8; 6] {
        [0, 0, 0, 0, 0, 0] // Loopback has no MAC address
    }
}
