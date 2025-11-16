//! 文件系统模块
//!
//! 包含文件系统相关的实现
//! 包括文件系统接口、文件操作等
//! 目前只实现了一个简单的内存文件系统
pub mod simple_fs;
pub mod smfs;

use alloc::string::String;
use alloc::sync::Arc;
use lazy_static::lazy_static;

use crate::devices::RamDisk;
use crate::fs::simple_fs::SimpleFs;
use crate::fs::smfs::SimpleMemoryFileSystem;
use crate::println;
use crate::vfs::{MOUNT_TABLE, MountFlags};

// lazy_static! {
//     /// 根文件系统实例
//     /// 在系统初始化时创建
//     /// 只读文件系统，驻留在内存中，不用担心同步问题
//     pub static ref ROOT_FS: SimpleMemoryFileSystem = SimpleMemoryFileSystem::init();
// }

/// 嵌入的 simple_fs 镜像
/// 由 build.rs 在编译时生成
static SIMPLE_FS_IMAGE: &[u8] = include_bytes!(env!("SIMPLE_FS_IMAGE"));

/// 挂载 simple_fs 作为根文件系统
pub fn init_simple_fs() -> Result<(), crate::vfs::FsError> {
    // 1. 创建 RamDisk，从静态镜像初始化
    println!(
        "[SimpleFS] Creating RamDisk ({} bytes)",
        SIMPLE_FS_IMAGE.len()
    );
    let ramdisk = RamDisk::from_bytes(SIMPLE_FS_IMAGE.to_vec(), 512, 0);

    // 2. 在 RamDisk 上创建 SimpleFS
    println!("[SimpleFS] Mounting SimpleFS on RamDisk");
    let simplefs = SimpleFs::from_ramdisk(ramdisk)?;

    // 3. 挂载为根文件系统
    MOUNT_TABLE.mount(
        Arc::new(simplefs),
        "/",
        MountFlags::empty(),
        Some(String::from("ramdisk0")),
    )?;

    println!("[SimpleFS] Root filesystem mounted at /");

    // 4. 列出根目录内容（调试用）
    if let Ok(root_dentry) = crate::vfs::get_root_dentry() {
        println!("[SimpleFS] Root directory contents:");
        let inode = root_dentry.inode.clone();
        if let Ok(entries) = inode.readdir() {
            for entry in entries {
                println!("  - {} (type: {:?})", entry.name, entry.inode_type);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
