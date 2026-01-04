//! # 文件系统模块 (FS)
//!
//! 本模块提供了多种具体的文件系统实现，通过实现VFS的`FileSystem`和`Inode` trait与虚拟文件系统层集成。
//!
//! ## 支持的文件系统
//!
//! - **[tmpfs](tmpfs)**: 临时文件系统(纯内存)
//!   - 高性能，所有数据存储在内存中
//!   - 支持配置最大容量限制
//!   - 适用于`/tmp`、缓存等临时存储
//!
//! - **[procfs](proc)**: 进程信息伪文件系统
//!   - 动态生成进程和系统信息
//!   - 标准Linux `/proc` 接口
//!   - 只读，使用Generator模式
//!
//! - **[sysfs](sysfs)**: 系统设备伪文件系统
//!   - 导出设备树和属性
//!   - 标准Linux `/sys` 接口
//!   - 使用Builder模式构建设备树
//!
//! - **[ext4]**: Linux Ext4文件系统
//!   - 持久化存储支持
//!   - 通过块设备访问
//!   - 支持完整的读写操作
//!
//! - **[simple_fs]**: 简单测试文件系统
//!   - 编译时嵌入镜像
//!   - 快速启动，用于测试
//!   - 只读，预加载用户程序
//!
//! ## 文件系统初始化流程
//!
//! ```no_run
//! # use crate::fs::*;
//! # use crate::vfs::*;
//! // 1. 挂载根文件系统(Ext4或SimpleFS)
//! init_ext4_from_block_device()
//!     .or_else(|_| init_simple_fs())?;
//!
//! // 2. 创建必要的目录
//! let root = vfs::get_root_dentry()?;
//! root.inode.mkdir("dev", FileMode::S_IFDIR | FileMode::S_IRWXU)?;
//! root.inode.mkdir("proc", FileMode::S_IFDIR | FileMode::S_IRWXU)?;
//! root.inode.mkdir("sys", FileMode::S_IFDIR | FileMode::S_IRWXU)?;
//! root.inode.mkdir("tmp", FileMode::S_IFDIR | FileMode::S_IRWXU)?;
//!
//! // 3. 挂载伪文件系统
//! init_procfs()?;
//! init_sysfs()?;
//!
//! // 4. 挂载tmpfs
//! mount_tmpfs("/tmp", 64)?;  // 64MB
//!
//! // 5. 初始化设备文件
//! init_dev()?;
//! # Ok::<(), FsError>(())
//! ```
//!
//! ## 文档参考
//!
//! 详细文档请参阅: `document/fs/README.md`
//!
//! ## 架构说明
//!
//! FS模块位于VFS层之下，作为VFS的具体实现：
//!
//! ```text
//! 系统调用层 (sys_open, sys_read, ...)
//!      ↓
//! VFS抽象层 (File, Inode, FileSystem traits)
//!      ↓
//! FS实现层 (TmpFs, ProcFS, Ext4, ...) ← 本模块
//!      ↓
//! 设备层 (BlockDriver, CharDriver)
//! ```
pub mod ext4;
pub mod proc;
pub mod simple_fs;
pub mod smfs;
pub mod sysfs;
pub mod tmpfs;

use alloc::string::String;
use alloc::sync::Arc;

use crate::device::BLK_DRIVERS;
use crate::device::RamDisk;
use crate::fs::ext4::Ext4FileSystem;
use crate::fs::simple_fs::SimpleFs;
use crate::fs::tmpfs::TmpFs;
// use crate::fs::smfs::SimpleMemoryFileSystem;
use crate::pr_info;
use crate::vfs::dev::makedev;
use crate::vfs::devno::{blkdev_major, chrdev_major};
use crate::vfs::{FileMode, FsError, MOUNT_TABLE, MountFlags, vfs_lookup};

// pub fn init_ext4() -> Result<(), crate::vfs::FsError> {
//     unimplemented!()
// }

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
    pr_info!(
        "[SimpleFS] Creating RamDisk ({} bytes)",
        SIMPLE_FS_IMAGE.len()
    );
    let ramdisk = RamDisk::from_bytes(SIMPLE_FS_IMAGE.to_vec(), 512, 0);

    // 2. 在 RamDisk 上创建 SimpleFS
    pr_info!("[SimpleFS] Mounting SimpleFS on RamDisk");
    let simplefs = SimpleFs::from_ramdisk(ramdisk)?;

    // 3. 挂载为根文件系统
    MOUNT_TABLE.mount(
        Arc::new(simplefs),
        "/",
        MountFlags::empty(),
        Some(String::from("ramdisk0")),
    )?;

    pr_info!("[SimpleFS] Root filesystem mounted at /");

    // 4. 列出根目录内容（调试用）
    if let Ok(root_dentry) = crate::vfs::get_root_dentry() {
        pr_info!("[SimpleFS] Root directory contents:");
        let inode = root_dentry.inode.clone();
        if let Ok(entries) = inode.readdir() {
            for entry in entries {
                pr_info!("  - {} (type: {:?})", entry.name, entry.inode_type);
            }
        }
    }

    Ok(())
}

/// 从真实的块设备初始化 Ext4 文件系统
///
/// 尝试从第一个可用的块设备创建 Ext4 文件系统，并挂载为根文件系统
pub fn init_ext4_from_block_device() -> Result<(), crate::vfs::FsError> {
    use crate::config::{EXT4_BLOCK_SIZE, FS_IMAGE_SIZE, VIRTIO_BLK_SECTOR_SIZE};
    use crate::vfs::FsError;

    pr_info!("[Ext4] Initializing Ext4 filesystem from block device");

    // 1. 获取第一个块设备驱动
    let blk_drivers = BLK_DRIVERS.read();
    if blk_drivers.is_empty() {
        pr_info!("[Ext4] No block device found");
        return Err(FsError::NoDevice);
    }

    let block_driver = blk_drivers[0].clone();
    drop(blk_drivers); // 释放锁

    pr_info!("[Ext4] Using block device: {}", block_driver.get_id());

    // 2. 获取块设备信息
    // Ext4 文件系统块大小 (必须与 mkfs.ext4 -b 参数一致)
    let ext4_block_size = EXT4_BLOCK_SIZE;
    // fs.img 大小 (由 qemu-run.sh 创建)
    // 计算总块数 (以 Ext4 块为单位, 而非扇区)
    let total_blocks = FS_IMAGE_SIZE / ext4_block_size;

    pr_info!(
        "[Ext4] Ext4 block size: {}, Total blocks: {}, Image size: {} MB",
        ext4_block_size,
        total_blocks,
        FS_IMAGE_SIZE / 1024 / 1024
    );

    // 3. 创建 Ext4 文件系统
    // 注意: BlockDeviceAdapter 内部必须使用 EXT4_BLOCK_SIZE (4096)
    let ext4_fs = Ext4FileSystem::open(block_driver, ext4_block_size, total_blocks, 0)?;

    // 4. 挂载为根文件系统
    pr_info!("[Ext4] Mounting Ext4 as root filesystem");
    MOUNT_TABLE.mount(
        ext4_fs,
        "/",
        MountFlags::empty(),
        Some(String::from("virtio-blk0")),
    )?;

    pr_info!("[Ext4] Root filesystem mounted at /");

    // 5. 列出根目录内容（调试用）
    if let Ok(root_dentry) = crate::vfs::get_root_dentry() {
        pr_info!("[Ext4] Root directory contents:");
        let inode = root_dentry.inode.clone();
        if let Ok(entries) = inode.readdir() {
            for entry in entries {
                pr_info!("  - {} (type: {:?})", entry.name, entry.inode_type);
            }
        } else {
            pr_info!("[Ext4] Failed to read root directory");
        }
    }

    Ok(())
}

/// 挂载 tmpfs 到指定路径
///
/// # 参数
///
/// - `mount_point`: 挂载点路径（如 "/tmp"）
/// - `max_size_mb`: 最大容量（MB），0 表示无限制
pub fn mount_tmpfs(mount_point: &str, max_size_mb: usize) -> Result<(), crate::vfs::FsError> {
    use crate::vfs::FsError;
    use alloc::string::ToString;

    pr_info!(
        "[Tmpfs] Creating tmpfs filesystem (max_size: {} MB)",
        if max_size_mb == 0 {
            "unlimited".to_string()
        } else {
            max_size_mb.to_string()
        }
    );

    // 创建 tmpfs
    let tmpfs = TmpFs::new(max_size_mb);

    // 挂载到指定路径
    MOUNT_TABLE.mount(
        tmpfs,
        mount_point,
        MountFlags::empty(),
        Some(String::from("tmpfs")),
    )?;

    pr_info!("[Tmpfs] Tmpfs mounted at {}", mount_point);

    Ok(())
}

pub fn init_dev() -> Result<(), FsError> {
    if let Err(e) = vfs_lookup("/dev") {
        return Err(e);
    }

    create_devices()?;

    Ok(())
}

fn create_devices() -> Result<(), FsError> {
    // 获取 /dev 目录的 dentry
    let dev_dentry = vfs_lookup("/dev")?;

    let dev_inode = &dev_dentry.inode;

    // 字符设备：0666 权限
    let char_mode = FileMode::S_IFCHR | FileMode::from_bits_truncate(0o666);

    // /dev/null (1, 3)
    dev_inode.mknod("null", char_mode, makedev(chrdev_major::MEM, 3))?;

    // /dev/zero (1, 5)
    dev_inode.mknod("zero", char_mode, makedev(chrdev_major::MEM, 5))?;

    // /dev/random (1, 8)
    dev_inode.mknod("random", char_mode, makedev(chrdev_major::MEM, 8))?;

    // /dev/urandom (1, 9)
    dev_inode.mknod("urandom", char_mode, makedev(chrdev_major::MEM, 9))?;

    // /dev/tty (5, 0) - 当前控制终端的别名（这里简化为同控制台驱动）
    let console_mode = FileMode::S_IFCHR | FileMode::from_bits_truncate(0o600);
    dev_inode.mknod("tty", console_mode, makedev(chrdev_major::CONSOLE, 0))?;
    // /dev/console (5, 1) - 只读
    dev_inode.mknod("console", console_mode, makedev(chrdev_major::CONSOLE, 1))?;

    // /dev/ttyS0 (4, 64)
    dev_inode.mknod("ttyS0", char_mode, makedev(chrdev_major::TTY, 64))?;

    // 创建 /dev/misc 目录
    let dir_mode = FileMode::S_IFDIR | FileMode::from_bits_truncate(0o755);
    dev_inode.mkdir("misc", dir_mode)?;

    // /dev/misc/rtc (10, 135)
    let misc_dentry = vfs_lookup("/dev/misc")?;
    misc_dentry
        .inode
        .mknod("rtc", char_mode, makedev(chrdev_major::MISC, 135))?;

    // 块设备：0660 权限
    let block_mode = FileMode::S_IFBLK | FileMode::from_bits_truncate(0o660);

    // /dev/vda (254, 0)
    dev_inode.mknod("vda", block_mode, makedev(blkdev_major::VIRTIO_BLK, 0))?;

    Ok(())
}

/// 初始化并挂载 procfs 到 /proc
pub fn init_procfs() -> Result<(), crate::vfs::FsError> {
    use crate::fs::proc::ProcFS;
    use crate::vfs::MountFlags;
    use alloc::string::ToString;

    pr_info!("[ProcFS] Initializing procfs");

    // 创建 procfs
    let procfs = ProcFS::new();

    // 初始化文件系统树
    procfs.init_tree()?;

    // 挂载到 /proc
    MOUNT_TABLE.mount(
        procfs,
        "/proc",
        MountFlags::empty(),
        Some(String::from("proc")),
    )?;

    pr_info!("[ProcFS] Procfs mounted at /proc");

    Ok(())
}

/// 初始化并挂载 sysfs 到 /sys
pub fn init_sysfs() -> Result<(), crate::vfs::FsError> {
    use crate::fs::sysfs::SysFS;
    use crate::vfs::MountFlags;
    use alloc::string::ToString;

    pr_info!("[SysFS] Initializing sysfs");

    // 创建 sysfs
    let sysfs = SysFS::new();

    // 初始化文件系统树 (从设备注册表构建设备树)
    sysfs.init_tree()?;

    // 挂载到 /sys
    MOUNT_TABLE.mount(
        sysfs,
        "/sys",
        MountFlags::empty(),
        Some(String::from("sysfs")),
    )?;

    pr_info!("[SysFS] Sysfs mounted at /sys");

    Ok(())
}

#[cfg(test)]
mod tests;
