//! 文件系统相关的用户空间 API 定义

use bitflags::bitflags;

/// 文件系统类型
///
/// 使用枚举避免魔数，同时保持与 Linux 魔数兼容
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum FileSystemType {
    /// EXT2/3/4 文件系统
    /// 魔数来自：include/uapi/linux/magic.h
    Ext4 = 0xEF53,

    /// 未知或不支持的文件系统
    Unknown = 0,
}

impl FileSystemType {
    /// 从文件系统类型字符串获取枚举值
    ///
    /// # 示例
    /// ```
    /// let fs_type = FileSystemType::from_str("ext4");
    /// assert_eq!(fs_type, FileSystemType::Ext4);
    /// ```
    pub fn from_str(fs_type: &str) -> Self {
        match fs_type {
            "ext4" | "ext3" | "ext2" => Self::Ext4,
            _ => Self::Unknown,
        }
    }

    /// 获取文件系统魔数
    ///
    /// # 返回值
    /// 返回 Linux 定义的文件系统魔数（i64）
    #[inline]
    pub fn magic(self) -> i64 {
        self as i64
    }
}

bitflags! {
    /// 文件系统挂载标志（用于 VFS 内部）
    ///
    /// 参考：include/uapi/linux/mount.h
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MountFlags: i64 {
        /// 只读挂载
        const RDONLY = 1;

        /// 忽略 suid/sgid 位
        const NOSUID = 2;

        /// 禁止访问设备文件
        const NODEV = 4;

        /// 禁止执行程序
        const NOEXEC = 8;

        /// 同步写入
        const SYNCHRONOUS = 16;

        /// 不更新访问时间
        const NOATIME = 1024;

        /// 不更新目录访问时间
        const NODIRATIME = 2048;

        /// 相对访问时间（修正值：与 Linux 内核一致）
        const RELATIME = 2097152;
    }
}

bitflags! {
    /// mount 系统调用标志位（与 Linux ABI 完全一致）
    ///
    /// 参考：include/uapi/linux/mount.h
    /// 这些标志在当前实现中会被忽略，但保留以保持 ABI 兼容性
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SysMountFlags: u64 {
        /// 只读挂载
        const MS_RDONLY      = 1;

        /// 禁止 SUID/SGID 位
        const MS_NOSUID      = 2;

        /// 禁止访问设备文件
        const MS_NODEV       = 4;

        /// 禁止执行程序
        const MS_NOEXEC      = 8;

        /// 同步所有写入
        const MS_SYNCHRONOUS = 16;

        /// 重新挂载（修改挂载选项）
        const MS_REMOUNT     = 32;

        /// 允许强制锁
        const MS_MANDLOCK    = 64;

        /// 目录同步
        const MS_DIRSYNC     = 128;

        /// 不跟随符号链接
        const MS_NOSYMFOLLOW = 256;

        /// 不更新访问时间
        const MS_NOATIME     = 1024;

        /// 不更新目录访问时间
        const MS_NODIRATIME  = 2048;

        /// 绑定挂载
        const MS_BIND        = 4096;

        /// 移动挂载
        const MS_MOVE        = 8192;

        /// 递归操作
        const MS_REC         = 16384;

        /// 相对访问时间
        const MS_RELATIME    = 2097152;

        /// 严格的访问时间
        const MS_STRICTATIME = 16777216;

        /// 延迟更新时间
        const MS_LAZYTIME    = 33554432;
    }
}

bitflags! {
    /// umount2 系统调用标志位（与 Linux ABI 完全一致）
    ///
    /// 参考：include/uapi/linux/mount.h
    /// 这些标志在当前实现中会被忽略，但保留以保持 ABI 兼容性
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct UmountFlags: i32 {
        /// 强制卸载（即使正在使用）
        const MNT_FORCE  = 1;

        /// 延迟卸载（不再使用时卸载）
        const MNT_DETACH = 2;

        /// 仅当挂载点过期时卸载
        const MNT_EXPIRE = 4;

        /// 不追踪符号链接
        const UMOUNT_NOFOLLOW = 8;
    }
}

bitflags! {
    /// 访问模式标志
    ///
    /// 用于 `faccessat` 系统调用
    /// 参考：include/uapi/asm-generic/fcntl.h
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AccessMode: i32 {
        /// 文件存在性测试（F_OK）
        const EXISTS = 0;

        /// 可执行性测试（X_OK）
        const EXECUTE = 1;

        /// 可写性测试（W_OK）
        const WRITE = 2;

        /// 可读性测试（R_OK）
        const READ = 4;
    }
}

// 兼容 Linux 常量名
pub const F_OK: i32 = 0;
pub const X_OK: i32 = 1;
pub const W_OK: i32 = 2;
pub const R_OK: i32 = 4;

bitflags! {
    /// AT_* 标志
    ///
    /// 用于 `*at` 系列系统调用（openat, fstatat, etc.）
    /// 参考：include/uapi/linux/fcntl.h
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AtFlags: u32 {
        /// 不跟随符号链接（AT_SYMLINK_NOFOLLOW）
        const SYMLINK_NOFOLLOW = 0x100;

        /// 使用有效 UID/GID 而非实际 UID/GID（AT_EACCESS）
        const EACCESS = 0x200;

        /// 移除目录（AT_REMOVEDIR，用于 unlinkat）
        const REMOVEDIR = 0x200;

        /// 路径为空时操作 dirfd 本身（AT_EMPTY_PATH）
        const EMPTY_PATH = 0x1000;

        /// 不触发自动挂载（AT_NO_AUTOMOUNT）
        const NO_AUTOMOUNT = 0x800;
    }
}

/// AT_FDCWD 常量
///
/// 表示使用当前工作目录作为相对路径的基准
pub const AT_FDCWD: i32 = -100;

bitflags! {
    /// renameat2 标志
    ///
    /// 参考：include/uapi/linux/fs.h
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RenameFlags: u32 {
        /// 目标存在时失败（RENAME_NOREPLACE）
        const NOREPLACE = 1 << 0;

        /// 原子交换两个文件（RENAME_EXCHANGE）
        const EXCHANGE = 1 << 1;

        /// 创建 whiteout 对象（RENAME_WHITEOUT，Union FS）
        const WHITEOUT = 1 << 2;
    }
}

impl RenameFlags {
    /// 检查标志组合是否合法
    ///
    /// NOREPLACE 和 EXCHANGE 不能同时设置
    pub fn is_valid(self) -> bool {
        !(self.contains(Self::NOREPLACE) && self.contains(Self::EXCHANGE))
    }
}

/// Linux statfs/statfs64 结构体（RISC-V 64位）
///
/// **重要**: 此结构体必须与 Linux 内核定义完全一致
///
/// 参考：include/uapi/asm-generic/statfs.h
///
/// # 字段说明
/// - `f_type`: 文件系统类型魔数（使用 FileSystemType 枚举）
/// - `f_bsize`: 最优传输块大小
/// - `f_blocks`: 文件系统总块数
/// - `f_bfree`: 空闲块数
/// - `f_bavail`: 非特权用户可用块数
/// - `f_files`: 总 inode 数
/// - `f_ffree`: 空闲 inode 数
/// - `f_fsid`: 文件系统 ID
/// - `f_namelen`: 最大文件名长度
/// - `f_frsize`: 片段大小
/// - `f_flags`: 挂载标志（MountFlags）
/// - `f_spare`: 保留字段（填充）
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LinuxStatFs {
    pub f_type: i64,
    pub f_bsize: i64,
    pub f_blocks: u64,
    pub f_bfree: u64,
    pub f_bavail: u64,
    pub f_files: u64,
    pub f_ffree: u64,
    pub f_fsid: [i32; 2],
    pub f_namelen: i64,
    pub f_frsize: i64,
    pub f_flags: i64,
    pub f_spare: [i64; 4],
}

impl LinuxStatFs {
    /// 创建零初始化的 statfs 结构
    pub const fn zeroed() -> Self {
        Self {
            f_type: 0,
            f_bsize: 0,
            f_blocks: 0,
            f_bfree: 0,
            f_bavail: 0,
            f_files: 0,
            f_ffree: 0,
            f_fsid: [0; 2],
            f_namelen: 0,
            f_frsize: 0,
            f_flags: 0,
            f_spare: [0; 4],
        }
    }
}

/// Linux stat 结构 (RISC-V 64位)
///
/// 必须与Linux内核的stat64结构完全匹配
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Stat {
    /// 设备ID
    pub st_dev: u64,

    /// Inode号
    pub st_ino: u64,

    /// 文件类型和权限
    pub st_mode: u32,

    /// 硬链接数
    pub st_nlink: u32,

    /// 用户ID
    pub st_uid: u32,

    /// 组ID
    pub st_gid: u32,

    /// 设备ID (如果是特殊文件)
    pub st_rdev: u64,

    pub __pad1: u64,

    /// 文件大小（字节）
    pub st_size: i64,

    /// 块大小
    pub st_blksize: i32,

    pub __pad2: i32,

    /// 占用的块数 (512B)
    pub st_blocks: i64,

    /// 访问时间（秒）
    pub st_atime_sec: i64,

    /// 访问时间（纳秒）
    pub st_atime_nsec: i64,

    /// 修改时间（秒）
    pub st_mtime_sec: i64,

    /// 修改时间（纳秒）
    pub st_mtime_nsec: i64,

    /// 状态改变时间（秒）
    pub st_ctime_sec: i64,

    /// 状态改变时间（纳秒）
    pub st_ctime_nsec: i64,

    pub __unused: [i32; 2],
}

/// Linux statx timestamp (与 Linux UAPI struct statx_timestamp 对齐)
///
/// 参考：include/uapi/linux/stat.h
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StatxTimestamp {
    pub tv_sec: i64,
    pub tv_nsec: u32,
    pub __reserved: i32,
}

impl StatxTimestamp {
    pub const fn zeroed() -> Self {
        Self {
            tv_sec: 0,
            tv_nsec: 0,
            __reserved: 0,
        }
    }
}

/// Linux statx 结构 (与 Linux UAPI struct statx 对齐)
///
/// 参考：include/uapi/linux/stat.h
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Statx {
    pub stx_mask: u32,
    pub stx_blksize: u32,
    pub stx_attributes: u64,
    pub stx_nlink: u32,
    pub stx_uid: u32,
    pub stx_gid: u32,
    pub stx_mode: u16,
    pub __spare0: [u16; 1],
    pub stx_ino: u64,
    pub stx_size: u64,
    pub stx_blocks: u64,
    pub stx_attributes_mask: u64,
    pub stx_atime: StatxTimestamp,
    pub stx_btime: StatxTimestamp,
    pub stx_ctime: StatxTimestamp,
    pub stx_mtime: StatxTimestamp,
    pub stx_rdev_major: u32,
    pub stx_rdev_minor: u32,
    pub stx_dev_major: u32,
    pub stx_dev_minor: u32,
    pub stx_mnt_id: u64,
    pub stx_dio_mem_align: u32,
    pub stx_dio_offset_align: u32,
    pub __spare3: [u64; 12],
}

impl Statx {
    pub const fn zeroed() -> Self {
        Self {
            stx_mask: 0,
            stx_blksize: 0,
            stx_attributes: 0,
            stx_nlink: 0,
            stx_uid: 0,
            stx_gid: 0,
            stx_mode: 0,
            __spare0: [0; 1],
            stx_ino: 0,
            stx_size: 0,
            stx_blocks: 0,
            stx_attributes_mask: 0,
            stx_atime: StatxTimestamp::zeroed(),
            stx_btime: StatxTimestamp::zeroed(),
            stx_ctime: StatxTimestamp::zeroed(),
            stx_mtime: StatxTimestamp::zeroed(),
            stx_rdev_major: 0,
            stx_rdev_minor: 0,
            stx_dev_major: 0,
            stx_dev_minor: 0,
            stx_mnt_id: 0,
            stx_dio_mem_align: 0,
            stx_dio_offset_align: 0,
            __spare3: [0; 12],
        }
    }
}

// statx mask bits (参考 include/uapi/linux/stat.h)
pub const STATX_TYPE: u32 = 0x0000_0001;
pub const STATX_MODE: u32 = 0x0000_0002;
pub const STATX_NLINK: u32 = 0x0000_0004;
pub const STATX_UID: u32 = 0x0000_0008;
pub const STATX_GID: u32 = 0x0000_0010;
pub const STATX_ATIME: u32 = 0x0000_0020;
pub const STATX_MTIME: u32 = 0x0000_0040;
pub const STATX_CTIME: u32 = 0x0000_0080;
pub const STATX_INO: u32 = 0x0000_0100;
pub const STATX_SIZE: u32 = 0x0000_0200;
pub const STATX_BLOCKS: u32 = 0x0000_0400;
pub const STATX_BASIC_STATS: u32 = 0x0000_07ff;

/// Linux dirent64 结构
///
/// 用于getdents64系统调用
#[repr(C)]
pub struct LinuxDirent64 {
    /// Inode号
    pub d_ino: u64,

    /// 到下一个dirent的偏移
    pub d_off: i64,

    /// 这个dirent的长度
    pub d_reclen: u16,

    /// 文件类型
    pub d_type: u8,
    // d_name: [u8]  // 文件名（变长，以\0结尾）
}

impl LinuxDirent64 {
    const BASE_SIZE: usize = 19;

    /// 计算包含文件名的总长度（8字节对齐）
    pub fn total_len(name: &str) -> usize {
        let len = Self::BASE_SIZE + name.len() + 1; // +1 for null terminator
        (len + 7) & !7 // 8字节对齐
    }
}
