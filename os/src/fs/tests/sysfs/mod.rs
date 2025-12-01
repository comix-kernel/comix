use crate::fs::sysfs::SysFS;
use crate::vfs::{FileMode, FileSystem, FsError, Inode, InodeType};
use alloc::sync::Arc;

// Test helper functions

/// Create a test sysfs filesystem
pub fn create_test_sysfs() -> Arc<SysFS> {
    SysFS::new()
}

/// Create and initialize a test sysfs filesystem with full tree
pub fn create_test_sysfs_with_tree() -> Result<Arc<SysFS>, FsError> {
    let sysfs = SysFS::new();
    sysfs.init_tree()?;
    Ok(sysfs)
}

// Export test modules
pub mod sysfs_attribute;
pub mod sysfs_basic;
pub mod sysfs_builders;
pub mod sysfs_device_registry;
pub mod sysfs_hierarchy;
pub mod sysfs_integration;
pub mod sysfs_symlink;
