pub mod block_device;
pub mod ram_disk;

pub use block_device::{BlockDevice, BlockError};
pub use ram_disk::RamDisk;

#[cfg(test)]
mod tests;