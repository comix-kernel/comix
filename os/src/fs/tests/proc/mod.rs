use crate::fs::proc::ProcFS;
use crate::vfs::{FileMode, FileSystem, FsError, Inode, InodeType};
use alloc::sync::Arc;

// Test helper functions

/// Create a test procfs filesystem
pub fn create_test_procfs() -> Arc<ProcFS> {
    ProcFS::new()
}

/// Create and initialize a test procfs filesystem with full tree
pub fn create_test_procfs_with_tree() -> Result<Arc<ProcFS>, FsError> {
    let procfs = ProcFS::new();
    procfs.init_tree()?;
    Ok(procfs)
}

// Export test modules
pub mod proc_basic;
pub mod proc_directory;
pub mod proc_dynamic_file;
pub mod proc_integration;
pub mod proc_symlink;
