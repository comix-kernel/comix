use alloc::sync::Arc;

use super::net_device::{NetDevice, NetDeviceError};

/// 一个“空”网络设备：不收包、不发包（发送直接丢弃），仅用于让 smoltcp 能初始化起来。
///
/// Comix 的 loopback 目前走 `NetDeviceAdapter` 内部的 loopback_queue，
/// 因此只要有一个可用的 Device/Interface，就能让 127.0.0.1 的 TCP/UDP 在内核内自洽跑通。
pub struct NullNetDevice {
    device_id: usize,
    name: &'static str,
    mac: [u8; 6],
    mtu: usize,
}

impl NullNetDevice {
    pub fn new(device_id: usize) -> Arc<Self> {
        Arc::new(Self {
            device_id,
            name: "null-net",
            // locally administered MAC
            mac: [0x02, 0x00, 0x00, 0x00, 0x00, 0x01],
            mtu: 1500,
        })
    }
}

impl NetDevice for NullNetDevice {
    fn send(&self, _packet: &[u8]) -> Result<(), NetDeviceError> {
        // drop
        Ok(())
    }

    fn receive(&self, _buf: &mut [u8]) -> Result<usize, NetDeviceError> {
        Err(NetDeviceError::QueueEmpty)
    }

    fn device_id(&self) -> usize {
        self.device_id
    }

    fn mtu(&self) -> usize {
        self.mtu
    }

    fn name(&self) -> &str {
        self.name
    }

    fn mac_address(&self) -> [u8; 6] {
        self.mac
    }
}
