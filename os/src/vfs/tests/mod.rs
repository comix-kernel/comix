use super::*;
use crate::device::block::ram_disk::RamDisk;
use crate::fs::simple_fs::SimpleFs;
use crate::vfs::file_system::FileSystem;
use alloc::string::String;
use alloc::sync::Arc;

// 测试辅助函数 (fixtures)

/// 创建一个空的测试用 SimpleFS 实例
pub fn create_test_simplefs() -> Arc<SimpleFs> {
    SimpleFs::new()
}

/// 创建一个指定大小的测试用 RamDisk
pub fn create_test_ramdisk(size_in_blocks: usize) -> Arc<RamDisk> {
    let block_size = 512;
    let total_size = size_in_blocks * block_size;
    RamDisk::new(total_size, block_size, 0)
}

/// 在测试 SimpleFS 中创建一个文件并写入内容
pub fn create_test_file_with_content(
    fs: &Arc<SimpleFs>,
    path: &str,
    content: &[u8],
) -> Result<Arc<dyn Inode>, FsError> {
    let root = fs.root_inode();
    let inode = root.create(path, FileMode::from_bits_truncate(0o644))?;
    inode.write_at(0, content)?;
    Ok(inode)
}

/// 在测试 SimpleFS 中创建一个目录
pub fn create_test_dir(fs: &Arc<SimpleFs>, path: &str) -> Result<Arc<dyn Inode>, FsError> {
    let root = fs.root_inode();
    root.mkdir(path, FileMode::from_bits_truncate(0o755))
}

/// 从 Inode 创建一个 Dentry (用于测试)
pub fn create_test_dentry(name: &str, inode: Arc<dyn Inode>) -> Arc<Dentry> {
    Dentry::new(String::from(name), inode)
}

/// 创建一个测试用的 File 对象
pub fn create_test_file(name: &str, inode: Arc<dyn Inode>, flags: OpenFlags) -> Arc<dyn File> {
    let dentry = create_test_dentry(name, inode);
    Arc::new(RegFile::new(dentry, flags))
}

pub mod blk_dev_file;
pub mod char_dev_file;
pub mod dentry;
pub mod devno;
pub mod fd_table;
pub mod file;
pub mod mount;
pub mod path;
pub mod pipe;
pub mod stdio;
pub mod trait_file;
