/// 网络设备错误
#[derive(Debug)]
pub enum NetDeviceError {
    IoError,
    DeviceNotReady,
    NotSupported,
    QueueFull,
    QueueEmpty,
    AllocationFailed,
}

/// 网络设备接口
pub trait NetDevice: Send + Sync {
    /// 发送数据包
    fn send(&self, packet: &[u8]) -> Result<(), NetDeviceError>;

    /// 接收数据包
    fn receive(&self, buf: &mut [u8]) -> Result<usize, NetDeviceError>;

    /// 获取设备标识符
    fn device_id(&self) -> usize;

    /// 获取最大传输单元(MTU)
    fn mtu(&self) -> usize;

    /// 获取设备名称
    fn name(&self) -> &str;

    /// 获取MAC地址
    fn mac_address(&self) -> [u8; 6];
}

use crate::device::virtio_hal::VirtIOHal;
use crate::sync::SpinLock;
use alloc::boxed::Box;
use alloc::sync::Arc;
use virtio_drivers::{
    device::net::{TxBuffer, VirtIONet},
    transport::Transport,
};

/// 使用 virtio-drivers 0.12.0 实现的 Virtio 网络设备
pub struct VirtioNetDevice<T: Transport + Send + Sync> {
    // 使用SpinLock包装UnsafeCell以实现线程安全的内部可变性
    virtio_net: SpinLock<Option<Box<VirtIONet<VirtIOHal, T, 256>>>>,
    device_id: usize,
    name: &'static str,
    mac: [u8; 6],
    mtu: usize,
}

impl<T: Transport + Send + Sync> VirtioNetDevice<T> {
    /// 创建新的 Virtio 网络设备实例
    ///
    /// # 参数
    /// * `transport` - VirtIO 设备传输层
    /// * `device_id` - 设备标识符
    pub fn new(transport: T, device_id: usize) -> Result<Arc<Self>, NetDeviceError> {
        let virtio_net = VirtIONet::<VirtIOHal, T, 256>::new(transport, 0)
            .map_err(|_| NetDeviceError::DeviceNotReady)?;

        // 从 VirtIONet 中获取 MAC 地址和 MTU
        let mac = virtio_net.mac_address();
        let mtu = 1500; // 默认以太网 MTU

        Ok(Arc::new(Self {
            virtio_net: SpinLock::new(Some(Box::new(virtio_net))),
            device_id,
            name: "virtio-net",
            mac,
            mtu,
        }))
    }
}

impl<T: Transport + Send + Sync> NetDevice for VirtioNetDevice<T> {
    /// 发送数据包
    fn send(&self, packet: &[u8]) -> Result<(), NetDeviceError> {
        if packet.len() > self.mtu {
            // 18 字节用于以太网头部和尾部
            return Err(NetDeviceError::QueueFull);
        }

        // 使用SpinLock的RAII模式，自动处理锁的获取和释放
        if let Some(ref mut virtio_net) = *self.virtio_net.lock() {
            let tx_buffer = TxBuffer::from(packet);
            match virtio_net.send(tx_buffer) {
                Ok(_) => Ok(()),
                Err(_) => Err(NetDeviceError::QueueFull),
            }
        } else {
            Err(NetDeviceError::DeviceNotReady)
        }
    }

    /// 接收数据包
    fn receive(&self, buf: &mut [u8]) -> Result<usize, NetDeviceError> {
        // 使用SpinLock的RAII模式，自动处理锁的获取和释放
        if let Some(ref mut virtio_net) = *self.virtio_net.lock() {
            match virtio_net.receive() {
                Ok(rx_buffer) => {
                    // 使用packet()方法获取实际的网络数据包（不包括头部）
                    let packet = rx_buffer.packet();
                    let actual_len = core::cmp::min(packet.len(), buf.len());
                    buf[..actual_len].copy_from_slice(&packet[..actual_len]);
                    Ok(actual_len)
                }
                Err(_) => Err(NetDeviceError::QueueEmpty),
            }
        } else {
            Err(NetDeviceError::DeviceNotReady)
        }
    }

    /// 获取设备标识符
    fn device_id(&self) -> usize {
        self.device_id
    }

    /// 获取最大传输单元(MTU)
    fn mtu(&self) -> usize {
        self.mtu
    }

    /// 获取设备名称
    fn name(&self) -> &str {
        self.name
    }

    /// 获取MAC地址
    fn mac_address(&self) -> [u8; 6] {
        self.mac
    }
}
