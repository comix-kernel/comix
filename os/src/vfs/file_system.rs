use crate::vfs::{FsError, Inode};
use alloc::string::String;
use alloc::sync::Arc;

/// 文件系统 trait
///
/// 所有文件系统实现都必须实现此 trait
pub trait FileSystem: Send + Sync {
    /// 文件系统类型名称
    ///
    /// # 示例
    /// - "ext4"
    fn fs_type(&self) -> &'static str;

    /// 获取根 inode
    ///
    /// 返回文件系统的根目录 inode
    fn root_inode(&self) -> Arc<dyn Inode>;

    /// 同步文件系统
    ///
    /// 将所有未写入的数据刷新到持久化存储
    fn sync(&self) -> Result<(), FsError>;

    /// 获取文件系统统计信息
    fn statfs(&self) -> Result<StatFs, FsError>;

    /// 卸载文件系统（可选）
    ///
    /// 执行卸载前的清理工作
    fn umount(&self) -> Result<(), FsError> {
        self.sync()
    }
}

/// 文件系统统计信息
#[derive(Debug, Clone)]
pub struct StatFs {
    /// 块大小（单位：字节）
    pub block_size: usize,

    /// 总块数
    pub total_blocks: usize,

    /// 空闲块数
    pub free_blocks: usize,

    /// 可用块数（非特权用户）
    pub available_blocks: usize,

    /// 总 inode 数
    pub total_inodes: usize,

    /// 空闲 inode 数
    pub free_inodes: usize,

    /// 文件系统 ID
    pub fsid: u64,

    /// 最大文件名长度
    pub max_filename_len: usize,
}
