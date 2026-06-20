//! 文件系统相关的系统调用实现

use core::ffi::c_char;

use crate::arch::Arch;

use crate::{
    kernel::{
        current_task,
        syscall::util::{
            create_file_at, create_file_from_dentry, get_path_safe, resolve_at_path,
            resolve_at_path_string, resolve_at_path_with_flags,
        },
    },
    uapi::{
        errno::{EACCES, EINVAL, ENOENT},
        fs::{AtFlags, F_OK, FileSystemType, LinuxStatFs, R_OK, W_OK, X_OK},
        time::TimeSpec,
    },
    util::user_buffer::write_to_user,
    vfs::{
        DENTRY_CACHE, Dentry, FileMode, FsError, InodeType, OpenFlags, SeekWhence, Stat, Statx,
        split_path, vfs_lookup,
    },
};

pub const AT_FDCWD: i32 = -100;
pub const AT_SYMLINK_NOFOLLOW: u32 = 0x100;
pub const AT_REMOVEDIR: u32 = 0x200;
pub const O_CLOEXEC: u32 = 0o2000000;

mod fd_ops;
mod metadata_ops;
mod mount_ops;
mod path_ops;
mod rename_ops;
mod stat_ops;

pub use fd_ops::*;
pub use metadata_ops::*;
pub use mount_ops::*;
pub use path_ops::*;
pub use rename_ops::*;
pub use stat_ops::*;
