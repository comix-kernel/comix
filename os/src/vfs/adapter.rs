//! linux系统调用的适配器
//!
//! 用于处理数据结构之间的转换

use super::{InodeMetadata, InodeType};

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

    __pad1: u64,

    /// 文件大小（字节）
    pub st_size: i64,

    /// 块大小
    pub st_blksize: i32,

    __pad2: i32,

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

    __unused: [i32; 2],
}

impl Stat {
    /// 从InodeMetadata创建Stat结构
    pub fn from_metadata(meta: &InodeMetadata) -> Self {
        Self {
            st_dev: 0, // TODO: 需要从文件系统获取设备号
            st_ino: meta.inode_no as u64,
            st_mode: meta.mode.bits(),
            st_nlink: meta.nlinks as u32,
            st_uid: meta.uid,
            st_gid: meta.gid,
            st_rdev: 0,
            __pad1: 0,
            st_size: meta.size as i64,
            st_blksize: 512,
            __pad2: 0,
            st_blocks: meta.blocks as i64,
            st_atime_sec: meta.atime.tv_sec,
            st_atime_nsec: meta.atime.tv_nsec,
            st_mtime_sec: meta.mtime.tv_sec,
            st_mtime_nsec: meta.mtime.tv_nsec,
            st_ctime_sec: meta.ctime.tv_sec,
            st_ctime_nsec: meta.ctime.tv_nsec,
            __unused: [0; 2],
        }
    }
}

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
    const BASE_SIZE: usize = core::mem::size_of::<Self>();

    /// 计算包含文件名的总长度（8字节对齐）
    pub fn total_len(name: &str) -> usize {
        let len = Self::BASE_SIZE + name.len() + 1; // +1 for null terminator
        (len + 7) & !7 // 8字节对齐
    }
}

/// 将InodeType转换为d_type值
pub fn inode_type_to_d_type(t: InodeType) -> u8 {
    match t {
        InodeType::File => 8,        // DT_REG
        InodeType::Directory => 4,   // DT_DIR
        InodeType::Symlink => 10,    // DT_LNK
        InodeType::CharDevice => 2,  // DT_CHR
        InodeType::BlockDevice => 6, // DT_BLK
        InodeType::Fifo => 1,        // DT_FIFO
        InodeType::Socket => 12,     // DT_SOCK
    }
}
