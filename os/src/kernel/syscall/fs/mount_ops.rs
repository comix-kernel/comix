use super::*;

/// mount - 挂载文件系统
///
/// # 系统调用号
/// 40 (SYS_MOUNT)
///
/// # 简化实现说明
/// - 只支持 ext4 文件系统（忽略 filesystemtype 参数）
/// - 使用第一个可用的块设备（忽略 source 参数）
/// - 忽略所有 mountflags（但保留以保持 ABI 兼容）
/// - 忽略 data 参数
pub fn mount(
    source: *const c_char,
    target: *const c_char,
    filesystemtype: *const c_char,
    _mountflags: u64,
    _data: *const core::ffi::c_void,
) -> isize {
    use crate::config::EXT4_BLOCK_SIZE;
    use crate::fs::ext4::Ext4FileSystem;
    use crate::fs::sysfs::find_block_device;
    use crate::fs::{init_dev, init_procfs, init_sysfs, mount_tmpfs};
    use crate::vfs::{MOUNT_TABLE, MountFlags as VfsMountFlags};
    use alloc::string::String;

    // 解析目标路径
    let target_str = match get_path_safe(target as usize) {
        Ok(s) => s,
        Err(_) => {
            return FsError::InvalidArgument.to_errno();
        }
    };

    // 解析 source (可能为空)
    let source_str = if !source.is_null() {
        match get_path_safe(source as usize) {
            Ok(s) => s,
            Err(_) => {
                return FsError::InvalidArgument.to_errno();
            }
        }
    } else {
        String::new()
    };

    // 解析 filesystemtype (可能为空)
    let fstype_str = if !filesystemtype.is_null() {
        match get_path_safe(filesystemtype as usize) {
            Ok(s) => s,
            Err(_) => {
                return FsError::InvalidArgument.to_errno();
            }
        }
    } else {
        String::new()
    };

    crate::pr_debug!(
        "[SYSCALL] mount: source='{}', target='{}', type='{}'",
        source_str,
        target_str,
        fstype_str
    );

    fn ensure_dir_exists(path: &str) -> Result<(), FsError> {
        use crate::vfs::{FileMode, split_path, vfs_lookup};

        match vfs_lookup(path) {
            Ok(dentry) => {
                let meta = dentry.inode.metadata()?;
                if meta.inode_type != InodeType::Directory {
                    return Err(FsError::NotDirectory);
                }
                Ok(())
            }
            Err(FsError::NotFound) => {
                let (parent_path, name) = split_path(path)?;
                let parent = vfs_lookup(&parent_path)?;
                let dir_mode = FileMode::S_IFDIR | FileMode::from_bits_truncate(0o755);
                parent.inode.mkdir(&name, dir_mode)?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    // 特殊挂载点处理
    match target_str.as_str() {
        "/proc" => {
            if let Err(e) = ensure_dir_exists("/proc") {
                return e.to_errno();
            }
            return match init_procfs() {
                Ok(_) => 0,
                Err(e) => e.to_errno(),
            };
        }
        "/sys" => {
            if let Err(e) = ensure_dir_exists("/sys") {
                return e.to_errno();
            }
            return match init_sysfs() {
                Ok(_) => 0,
                Err(e) => e.to_errno(),
            };
        }
        "/tmp" => {
            if let Err(e) = ensure_dir_exists("/tmp") {
                return e.to_errno();
            }
            return match mount_tmpfs("/tmp", 0) {
                Ok(_) => 0,
                Err(e) => e.to_errno(),
            };
        }
        "/dev" => {
            if let Err(e) = ensure_dir_exists("/dev") {
                return e.to_errno();
            }
            // 先挂载 tmpfs 到 /dev
            if let Err(e) = mount_tmpfs("/dev", 0) {
                return e.to_errno();
            }
            // 然后初始化设备节点
            return match init_dev() {
                Ok(_) => 0,
                Err(e) => e.to_errno(),
            };
        }
        _ => {}
    }

    // 通用挂载逻辑 (目前只支持 ext4)
    if fstype_str == "ext4" {
        // 查找块设备
        let dev_info = match find_block_device(&source_str) {
            Some(info) => info,
            None => {
                crate::pr_err!("[SYSCALL] mount: block device '{}' not found", source_str);
                return -(ENOENT as isize);
            }
        };

        let block_device = dev_info.device;

        // 创建 Ext4 文件系统
        let block_size = EXT4_BLOCK_SIZE;
        // 注意：这里我们无法直接知道设备的总块数，
        // 但 Ext4FileSystem::open 通常会读取超级块来获取这些信息，
        // 或者我们可以从 block_device 获取容量。
        // 暂时使用 fs.img 的默认大小计算，或者让 Ext4FileSystem 自己处理。
        // 为了兼容现有 API，我们尝试从设备获取大小。
        let total_blocks = block_device.total_blocks();

        let ext4_fs = match Ext4FileSystem::open(block_device.clone(), block_size, total_blocks, 0)
        {
            Ok(fs) => fs,
            Err(e) => {
                crate::pr_err!("[SYSCALL] mount: failed to open ext4: {:?}", e);
                return e.to_errno();
            }
        };

        // 挂载文件系统
        match MOUNT_TABLE.mount(
            ext4_fs,
            &target_str,
            VfsMountFlags::empty(),
            Some(source_str),
        ) {
            Ok(()) => {
                crate::pr_info!(
                    "[SYSCALL] mount: successfully mounted ext4 at '{}'",
                    target_str
                );
                return 0;
            }
            Err(e) => {
                crate::pr_err!("[SYSCALL] mount: failed: {:?}", e);
                return e.to_errno();
            }
        }
    }

    crate::pr_err!(
        "[SYSCALL] mount: unsupported filesystem type '{}' or target '{}'",
        fstype_str,
        target_str
    );
    -(EINVAL as isize)
}

/// umount2 - 卸载文件系统
///
/// # 系统调用号
/// 39 (SYS_UMOUNT2)
///
/// # 简化实现说明
/// - 忽略 flags 参数（但保留以保持 ABI 兼容）
/// - 不检查文件是否被占用
/// - 直接调用 MOUNT_TABLE.umount()
pub fn umount2(target: *const c_char, _flags: i32) -> isize {
    use crate::vfs::MOUNT_TABLE;

    // 解析目标路径
    let target_str = match get_path_safe(target as usize) {
        Ok(s) => s,
        Err(_) => {
            return FsError::InvalidArgument.to_errno();
        }
    };

    crate::pr_debug!("[SYSCALL] umount2: unmounting '{}'", target_str);

    // 卸载文件系统

    // 注意：MOUNT_TABLE.umount() 会自动调用 fs.sync()
    match MOUNT_TABLE.umount(&target_str) {
        Ok(()) => {
            crate::pr_debug!("[SYSCALL] umount2: successfully unmounted '{}'", target_str);
            0
        }
        Err(e) => {
            crate::pr_err!("[SYSCALL] umount2: failed: {:?}", e);
            e.to_errno()
        }
    }
}

/// 同步所有文件系统
///
/// # 实现说明
/// 由于 Comix 使用写直达架构,数据已在块设备中,
/// 此调用只需刷新硬件写缓存。
pub fn sync() -> isize {
    use crate::kernel::syscall::util::flush_all_block_devices;

    // sync 总是成功(即使 flush 失败也不返回错误)
    let _ = flush_all_block_devices();
    0
}

/// syncfs - 同步指定文件系统
pub fn syncfs(fd: usize) -> isize {
    use crate::kernel::syscall::util::flush_block_device_by_fd;

    match flush_block_device_by_fd(fd) {
        Ok(()) => 0,
        Err(errno) => errno,
    }
}

/// fsync - 同步文件数据和元数据
///
/// # 实现说明
/// 在写直达架构下,等同于 syncfs
pub fn fsync(fd: usize) -> isize {
    syncfs(fd)
}

/// fdatasync - 同步文件数据(元数据可选)
///
/// # 实现说明
/// 在写直达架构下,完全等同于 fsync
pub fn fdatasync(fd: usize) -> isize {
    fsync(fd)
}
