pub mod dentry;
pub mod error;
pub mod fd_table;
pub mod file;
pub mod file_system;
pub mod inode;
pub mod mount;
pub mod path;
pub mod stdio;

pub use dentry::{DENTRY_CACHE, Dentry, DentryCache};
pub use error::FsError;
pub use fd_table::FDTable;
pub use file::{File, OpenFlags, SeekWhence};
pub use file_system::{FileSystem, StatFs};
pub use inode::{DirEntry, FileMode, Inode, InodeMetadata, InodeType, TimeSpec};
pub use mount::{MOUNT_TABLE, MountFlags, MountPoint, MountTable, get_root_dentry};
pub use path::{normalize_path, parse_path, split_path, vfs_lookup};
pub use stdio::{StderrInode, StdinInode, StdoutInode, create_stdio_files};

use alloc::{vec, vec::Vec};

pub fn vfs_load_elf(path: &str) -> Result<Vec<u8>, FsError> {
    let dentry = vfs_lookup(path)?;
    let inode = &dentry.inode;
    let metadata = inode.metadata()?;

    // 确保是普通文件
    if metadata.inode_type != InodeType::File {
        return Err(FsError::IsDirectory);
    }

    let mut buf = vec![0u8; metadata.size];
    inode.read_at(0, &mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests;