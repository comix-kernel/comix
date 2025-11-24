use crate::device::block::ram_disk::RamDisk;
use crate::fs::simple_fs::SimpleFs;
use crate::vfs::error::FsError;
use crate::vfs::file_system::FileSystem;
use crate::vfs::inode::{FileMode, Inode, InodeType};
use alloc::sync::Arc;

// 测试辅助函数 (fixtures)

/// 创建一个空的测试用 SimpleFS 实例
pub fn create_test_simplefs() -> Arc<SimpleFs> {
    SimpleFs::new()
}

/// 创建一个包含测试文件的 RamDisk
pub fn create_test_ramdisk_with_files() -> Arc<RamDisk> {
    // 创建一个简单的 RamDisk 镜像格式:
    // Header: "RAMDISK\0" (8 bytes) + file_count (4 bytes)
    // 为了测试,我们创建一个空的 ramdisk
    let mut data = alloc::vec![];

    // Magic: "RAMDISK\0"
    data.extend_from_slice(b"RAMDISK\0");

    // File count: 0 (little-endian)
    data.extend_from_slice(&0u32.to_le_bytes());

    // Pad to block size (512 bytes)
    data.resize(512, 0);

    RamDisk::from_bytes(data, 512, 0)
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

pub mod simple_fs_basic;
pub mod simple_fs_dir;
pub mod simple_fs_integration;
pub mod simple_fs_permission;
pub mod simple_fs_ramdisk;
