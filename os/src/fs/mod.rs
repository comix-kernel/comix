//! 文件系统模块
//!
//! 包含文件系统相关的实现
//! 包括文件系统接口、文件操作等
//! 目前只实现了一个简单的内存文件系统
pub mod smfs;

use lazy_static::lazy_static;

use crate::fs::smfs::SimpleMemoryFileSystem;

lazy_static! {
    /// 根文件系统实例
    /// 在系统初始化时创建
    /// 只读文件系统，驻留在内存中，不用担心同步问题
    pub static ref ROOT_FS: SimpleMemoryFileSystem = SimpleMemoryFileSystem::init();
}
