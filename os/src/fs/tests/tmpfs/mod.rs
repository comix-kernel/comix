use crate::fs::tmpfs::TmpFs;
use crate::vfs::{FileMode, FileSystem, FsError, Inode, InodeType};
use alloc::sync::Arc;

// Test helper functions (fixtures)

/// Create a test tmpfs filesystem (16 MB)
pub fn create_test_tmpfs() -> Arc<TmpFs> {
    TmpFs::new(16) // 16 MB
}

/// Create a test tmpfs with unlimited size
pub fn create_test_tmpfs_unlimited() -> Arc<TmpFs> {
    TmpFs::new(0)
}

/// Create a test tmpfs with small size (for capacity tests)
pub fn create_test_tmpfs_small() -> Arc<TmpFs> {
    TmpFs::new(1) // 1 MB
}

/// Create a file with content in test tmpfs
pub fn create_test_file_with_content(
    fs: &Arc<TmpFs>,
    name: &str,
    content: &[u8],
) -> Result<Arc<dyn Inode>, FsError> {
    let root = fs.root_inode();
    let inode = root.create(name, FileMode::from_bits_truncate(0o644))?;
    inode.write_at(0, content)?;
    Ok(inode)
}

/// Create a directory in test tmpfs
pub fn create_test_dir(fs: &Arc<TmpFs>, name: &str) -> Result<Arc<dyn Inode>, FsError> {
    let root = fs.root_inode();
    root.mkdir(name, FileMode::from_bits_truncate(0o755))
}

// Export test modules
pub mod tmpfs_basic;
pub mod tmpfs_capacity;
pub mod tmpfs_directory;
pub mod tmpfs_error;
pub mod tmpfs_integration;
pub mod tmpfs_io;
pub mod tmpfs_metadata;
pub mod tmpfs_sparse;
