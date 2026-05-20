use super::*;

/// Smoltcp interface wrapper that owns the adapter borrowed by `Interface`.
pub struct SmoltcpInterface {
    device_adapter: NetDeviceAdapter,
    iface: Interface,
}

impl SmoltcpInterface {
    /// Create a smoltcp interface wrapper for a net device.
    pub(crate) fn new(
        device: Arc<dyn crate::device::net::net_device::NetDevice>,
        mac_address: EthernetAddress,
    ) -> Self {
        let mut device_adapter = NetDeviceAdapter::new(device);
        let config =
            smoltcp::iface::Config::new(smoltcp::wire::HardwareAddress::Ethernet(mac_address));
        let current_time = crate::arch::get_time_ms() as i64;
        let iface = Interface::new(
            config,
            &mut device_adapter,
            Instant::from_millis(current_time),
        );

        Self {
            device_adapter,
            iface,
        }
    }

    /// Poll the interface and sockets.
    pub fn poll(
        &mut self,
        timestamp: Instant,
        sockets: &mut smoltcp::iface::SocketSet,
    ) -> smoltcp::iface::PollResult {
        self.iface
            .poll(timestamp, &mut self.device_adapter, sockets)
    }

    /// Mutable access for compatibility during the migration.
    pub fn interface_mut(&mut self) -> &mut Interface {
        &mut self.iface
    }

    /// Immutable access for compatibility during the migration.
    pub fn interface(&self) -> &Interface {
        &self.iface
    }

    /// Mutable adapter access for compatibility during the migration.
    pub fn device_adapter_mut(&mut self) -> &mut NetDeviceAdapter {
        &mut self.device_adapter
    }

    /// Consume the wrapper and return the smoltcp interface.
    pub fn into_interface(self) -> Interface {
        self.iface
    }
}

/// Adapter from Comix `NetDevice` to smoltcp's `Device` trait.
#[derive(Clone)]
pub struct NetDeviceAdapter {
    device: Arc<dyn crate::device::net::net_device::NetDevice>,
    rx_buffer: [u8; 2048],
}

impl NetDeviceAdapter {
    /// Create a new adapter.
    pub fn new(device: Arc<dyn crate::device::net::net_device::NetDevice>) -> Self {
        Self {
            device,
            rx_buffer: [0; 2048],
        }
    }

    /// Compatibility hook for the old bounded loopback drain path.
    pub fn loopback_queue_len(&self) -> usize {
        loopback_link_len()
    }
}

impl smoltcp::phy::Device for NetDeviceAdapter {
    type RxToken<'a> = NetRxToken<'a>;
    type TxToken<'a> = NetTxToken<'a>;

    fn receive(
        &mut self,
        _timestamp: smoltcp::time::Instant,
    ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        if let Some(packet) = dequeue_loopback_frame() {
            if packet.len() > self.rx_buffer.len() {
                return None;
            }
            self.rx_buffer[..packet.len()].copy_from_slice(&packet);
            return Some((
                NetRxToken {
                    buffer: &self.rx_buffer[..packet.len()],
                },
                NetTxToken {
                    device: &self.device,
                },
            ));
        }

        match self.device.receive(&mut self.rx_buffer) {
            Ok(size) if size > 0 => Some((
                NetRxToken {
                    buffer: &self.rx_buffer[..size],
                },
                NetTxToken {
                    device: &self.device,
                },
            )),
            _ => None,
        }
    }

    fn transmit(&mut self, _timestamp: smoltcp::time::Instant) -> Option<Self::TxToken<'_>> {
        Some(NetTxToken {
            device: &self.device,
        })
    }

    fn capabilities(&self) -> smoltcp::phy::DeviceCapabilities {
        let mut caps = smoltcp::phy::DeviceCapabilities::default();
        caps.max_transmission_unit =
            self.device.mtu() + smoltcp::wire::EthernetFrame::<&[u8]>::header_len();
        caps.medium = smoltcp::phy::Medium::Ethernet;
        caps.max_burst_size = Some(64);
        caps
    }
}

/// Receive token.
pub struct NetRxToken<'a> {
    buffer: &'a [u8],
}

impl smoltcp::phy::RxToken for NetRxToken<'_> {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(self.buffer)
    }

    fn meta(&self) -> smoltcp::phy::PacketMeta {
        smoltcp::phy::PacketMeta::default()
    }
}

/// Transmit token.
pub struct NetTxToken<'a> {
    device: &'a Arc<dyn crate::device::net::net_device::NetDevice>,
}

impl smoltcp::phy::TxToken for NetTxToken<'_> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = alloc::vec![0; len];
        let result = f(&mut buffer);

        let is_loopback = if buffer.len() >= 14 {
            let ethertype = u16::from_be_bytes([buffer[12], buffer[13]]);
            match ethertype {
                0x0800 if buffer.len() >= 34 => buffer[26] == 127 || buffer[30] == 127,
                0x0806 if buffer.len() >= 42 => buffer[28] == 127 || buffer[38] == 127,
                _ => false,
            }
        } else {
            false
        };

        if is_loopback {
            enqueue_loopback_frame(buffer);
        } else {
            let _ = self.device.send(&buffer);
        }

        result
    }

    fn set_meta(&mut self, _meta: smoltcp::phy::PacketMeta) {}
}
