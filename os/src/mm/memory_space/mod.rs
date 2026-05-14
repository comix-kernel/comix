//! 内存空间模块
//!
//! 本模块定义了内存空间（Memory Space）的相关结构和功能，
//! 包括内存空间的创建、管理以及与映射区域（Mapping Area）的交互。
pub mod mapping_area;
mod space;
mod mmap_file;

pub use space::*;
pub use mmap_file::MmapFile;
