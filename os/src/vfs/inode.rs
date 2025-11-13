use crate::arch::timer::get_time;
use crate::config::CLOCK_FREQ;
use crate::vfs::error::FsError;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

/// Inode类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InodeType {
    File,        // 普通文件
    Directory,   // 目录
    Symlink,     // 符号链接
    CharDevice,  // 字符设备
    BlockDevice, // 块设备
    Fifo,        // 命名管道
    Socket,      // 套接字
}

/// 时间戳结构
#[derive(Debug, Clone, Copy)]
pub struct TimeSpec {
    pub sec: i64,  // 秒
    pub nsec: i64, // 纳秒
}

impl TimeSpec {
    /// 创建当前时间戳
    pub fn now() -> Self {
        const NSEC_PER_SEC: usize = 1000_000_000;
        let cur_nsec = get_time() * NSEC_PER_SEC / CLOCK_FREQ;
        Self {
            sec: (cur_nsec / NSEC_PER_SEC) as i64,
            nsec: (cur_nsec % NSEC_PER_SEC) as i64,
        }
    }

    /// 创建零时间戳
    pub fn zero() -> Self {
        Self { sec: 0, nsec: 0 }
    }
}

/// 文件权限和类型（与 POSIX 兼容）
bitflags::bitflags! {
    pub struct FileMode: u32 {
        // 文件类型掩码
        const S_IFMT   = 0o170000;  // 文件类型掩码
        const S_IFREG  = 0o100000;  // 普通文件
        const S_IFDIR  = 0o040000;  // 目录
        const S_IFLNK  = 0o120000;  // 符号链接
        const S_IFCHR  = 0o020000;  // 字符设备
        const S_IFBLK  = 0o060000;  // 块设备
        const S_IFIFO  = 0o010000;  // FIFO
        const S_IFSOCK = 0o140000;  // Socket

        // 用户权限
        const S_IRUSR  = 0o400;     // 用户读
        const S_IWUSR  = 0o200;     // 用户写
        const S_IXUSR  = 0o100;     // 用户执行

        // 组权限
        const S_IRGRP  = 0o040;     // 组读
        const S_IWGRP  = 0o020;     // 组写
        const S_IXGRP  = 0o010;     // 组执行

        // 其他用户权限
        const S_IROTH  = 0o004;     // 其他读
        const S_IWOTH  = 0o002;     // 其他写
        const S_IXOTH  = 0o001;     // 其他执行

        // 特殊位
        const S_ISUID  = 0o4000;    // Set UID
        const S_ISGID  = 0o2000;    // Set GID
        const S_ISVTX  = 0o1000;    // Sticky bit
    }
}

impl FileMode {
    /// 检查是否有读权限（暂时只检查用户权限）
    pub fn can_read(&self) -> bool {
        self.contains(FileMode::S_IRUSR)
    }

    /// 检查是否有写权限
    pub fn can_write(&self) -> bool {
        self.contains(FileMode::S_IWUSR)
    }

    /// 检查是否有执行权限
    pub fn can_execute(&self) -> bool {
        self.contains(FileMode::S_IXUSR)
    }
}

/// 轻量级目录项（readdir 返回）
///
/// 用于数据传输，无引用关系，读取后即可丢弃
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,          // 文件名
    pub inode_no: usize,       // Inode 编号
    pub inode_type: InodeType, // 文件类型
}

/// Inode 元数据
#[derive(Debug, Clone)]
pub struct InodeMetadata {
    pub inode_no: usize,       // Inode 编号
    pub inode_type: InodeType, // 文件类型
    pub mode: FileMode,        // 权限位
    pub uid: u32,              // 用户 ID
    pub gid: u32,              // 组 ID
    pub size: usize,           // 文件大小（字节）
    pub atime: TimeSpec,       // 访问时间
    pub mtime: TimeSpec,       // 修改时间
    pub ctime: TimeSpec,       // 状态改变时间
    pub nlinks: usize,         // 硬链接数
    pub blocks: usize,         // 占用的块数（512B 为单位）
}

/// Inode trait - 所有文件系统必须实现
pub trait Inode: Send + Sync {
    /// 获取 inode 元数据
    fn metadata(&self) -> Result<InodeMetadata, FsError>;

    /// 从指定偏移量读取数据
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError>;

    /// 从指定偏移量写入数据
    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError>;

    /// 在目录中查找子项
    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, FsError>;

    /// 在目录中创建文件
    fn create(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError>;

    /// 在目录中创建子目录
    fn mkdir(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError>;

    /// 删除目录项
    fn unlink(&self, name: &str) -> Result<(), FsError>;

    /// 列出目录内容
    fn readdir(&self) -> Result<Vec<DirEntry>, FsError>;

    /// 截断文件到指定大小
    fn truncate(&self, size: usize) -> Result<(), FsError>;

    /// 同步文件数据到存储设备
    fn sync(&self) -> Result<(), FsError>;
}
