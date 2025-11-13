//! 设备抽象层，提供块设备接口和内存磁盘实现

pub mod block_device;
pub mod ram_disk;

pub use block_device::{BlockDevice, BlockError};
pub use ram_disk::RamDisk;

#[cfg(test)]
mod tests;