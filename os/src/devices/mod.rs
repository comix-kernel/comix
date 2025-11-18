//! 设备抽象层，提供块设备接口、内存磁盘实现和网络设备接口

pub mod block_device;
pub mod net_device;
pub mod ram_disk;
pub mod virtio_hal;

pub use block_device::{BlockDevice, BlockError};
pub use net_device::NetDevice;
pub use ram_disk::RamDisk;
pub use virtio_hal::VirtIOHal;

#[cfg(test)]
mod tests;
