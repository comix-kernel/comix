//! FAT/VFAT filesystem support.

pub mod adapter;

mod fs;
mod inode;

pub use fs::VfatFileSystem;
