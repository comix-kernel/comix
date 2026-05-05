use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::sync::SpinLock;

use super::net_device::{NetDevice, NetDeviceError};

/// Explicit loopback network device.
///
/// Frames sent to this device are queued and returned by `receive()`, which
/// makes loopback a real device model instead of a `NullNetDevice` side effect.
pub struct LoopbackNetDevice {
    device_id: usize,
    rx_queue: SpinLock<VecDeque<Vec<u8>>>,
    mac: [u8; 6],
    mtu: usize,
}

impl LoopbackNetDevice {
    pub fn new(device_id: usize) -> Arc<Self> {
        Arc::new(Self {
            device_id,
            rx_queue: SpinLock::new(VecDeque::new()),
            mac: [0x02, 0x00, 0x00, 0x00, 0x00, 0x7f],
            mtu: 65535,
        })
    }
}

impl NetDevice for LoopbackNetDevice {
    fn send(&self, packet: &[u8]) -> Result<(), NetDeviceError> {
        self.rx_queue.lock().push_back(packet.to_vec());
        Ok(())
    }

    fn receive(&self, buf: &mut [u8]) -> Result<usize, NetDeviceError> {
        let Some(packet) = self.rx_queue.lock().pop_front() else {
            return Err(NetDeviceError::QueueEmpty);
        };
        let len = core::cmp::min(packet.len(), buf.len());
        buf[..len].copy_from_slice(&packet[..len]);
        Ok(len)
    }

    fn device_id(&self) -> usize {
        self.device_id
    }

    fn mtu(&self) -> usize {
        self.mtu
    }

    fn name(&self) -> &str {
        "loopback"
    }

    fn mac_address(&self) -> [u8; 6] {
        self.mac
    }
}
