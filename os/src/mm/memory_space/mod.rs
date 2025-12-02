//! 内存空间模块
//!
//! 本模块定义了内存空间（Memory Space）的相关结构和功能，
//! 包括内存空间的创建、管理以及与映射区域（Mapping Area）的交互。
//! HACK: 在一个模块目录/文件的顶层又声明了一个同名子模块，这会造成 "module inception"。
//! 虽然功能上可行，但会引起 API/模块层次混淆，Clippy 建议消除这种重复。
#![allow(clippy::module_inception)]
pub mod mapping_area;
mod memory_space;
mod mmap_file;

pub use memory_space::*;
pub use mmap_file::MmapFile;
