//! linux系统调用的适配器
//!
//! 用于处理数据结构之间的转换

use super::{InodeMetadata, InodeType};
use crate::uapi::fs::{LinuxDirent64, Stat};

/// Stat 结构适配方法
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
            st_rdev: meta.rdev,
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
