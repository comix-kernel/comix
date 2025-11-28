//! Tmpfs - 内存临时文件系统
//!
//! Tmpfs 是一个完全驻留在内存中的文件系统，数据存储在物理内存页中。
//! 适用于临时文件存储，重启后数据会丢失。
//!
//! # 设计特点
//!
//! - **按需分配**：文件数据页按需分配，节省内存
//! - **稀疏文件支持**：使用 `Vec<Option<Arc<FrameTracker>>>` 支持文件空洞
//! - **无 BlockDevice 层**：直接管理物理帧，性能更好
//! - **Arc 共享**：使用 Arc 实现多进程文件共享

mod inode;
mod tmpfs;

pub use inode::TmpfsInode;
pub use tmpfs::TmpFs;
