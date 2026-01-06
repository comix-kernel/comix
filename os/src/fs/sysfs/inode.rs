//! Sysfs Inode 实现
//!
//! 提供三种类型的 Inode:
//! - 目录 (Directory)
//! - 属性文件 (Attribute) - 动态生成内容
//! - 符号链接 (Symlink)

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::Any;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::sync::Mutex;
use crate::uapi::time::TimeSpec;
use crate::vfs::{DirEntry, FileMode, FsError, Inode, InodeMetadata, InodeType};

/// Sysfs 属性生成器
///
/// 用于动态生成属性文件的内容
pub type AttrShowFn = dyn Fn() -> Result<String, FsError> + Send + Sync;
pub type AttrStoreFn = dyn Fn(&str) -> Result<(), FsError> + Send + Sync;

/// Sysfs 属性
pub struct SysfsAttr {
    pub name: String,
    pub mode: FileMode,
    pub show: Arc<AttrShowFn>,
    pub store: Option<Arc<AttrStoreFn>>,
}

/// Sysfs Inode 内容类型
pub enum SysfsInodeContent {
    /// 目录 (子节点)
    Directory(Mutex<BTreeMap<String, Arc<SysfsInode>>>),

    /// 属性文件 (动态生成)
    Attribute(SysfsAttr),

    /// 符号链接
    Symlink(String),
}

/// Sysfs Inode
pub struct SysfsInode {
    inode_no: usize,
    inode_type: InodeType,
    metadata: Mutex<InodeMetadata>,
    content: SysfsInodeContent,
}

static NEXT_INODE_NO: AtomicUsize = AtomicUsize::new(1);

impl SysfsInode {
    /// 创建目录 inode
    pub fn new_directory(mode: FileMode) -> Arc<Self> {
        let inode_no = NEXT_INODE_NO.fetch_add(1, Ordering::Relaxed);
        let now = TimeSpec::now();
        Arc::new(Self {
            inode_no,
            inode_type: InodeType::Directory,
            metadata: Mutex::new(InodeMetadata {
                inode_no,
                inode_type: InodeType::Directory,
                size: 0,
                mode,
                nlinks: 2,
                uid: 0,
                gid: 0,
                atime: now,
                mtime: now,
                ctime: now,
                blocks: 0,
                rdev: 0,
            }),
            content: SysfsInodeContent::Directory(Mutex::new(BTreeMap::new())),
        })
    }

    /// 创建属性文件 inode
    pub fn new_attribute(attr: SysfsAttr) -> Arc<Self> {
        let inode_no = NEXT_INODE_NO.fetch_add(1, Ordering::Relaxed);
        let mode = attr.mode.clone();
        let now = TimeSpec::now();
        Arc::new(Self {
            inode_no,
            inode_type: InodeType::File,
            metadata: Mutex::new(InodeMetadata {
                inode_no,
                inode_type: InodeType::File,
                size: 0, // sysfs 文件大小总是 0
                mode,
                nlinks: 1,
                uid: 0,
                gid: 0,
                atime: now,
                mtime: now,
                ctime: now,
                blocks: 0,
                rdev: 0,
            }),
            content: SysfsInodeContent::Attribute(attr),
        })
    }

    /// 创建符号链接 inode
    pub fn new_symlink(target: String) -> Arc<Self> {
        let inode_no = NEXT_INODE_NO.fetch_add(1, Ordering::Relaxed);
        let now = TimeSpec::now();
        Arc::new(Self {
            inode_no,
            inode_type: InodeType::Symlink,
            metadata: Mutex::new(InodeMetadata {
                inode_no,
                inode_type: InodeType::Symlink,
                size: target.len(),
                mode: FileMode::from_bits_truncate(0o777),
                nlinks: 1,
                uid: 0,
                gid: 0,
                atime: now,
                mtime: now,
                ctime: now,
                blocks: 0,
                rdev: 0,
            }),
            content: SysfsInodeContent::Symlink(target),
        })
    }

    /// 向目录添加子节点
    pub fn add_child(&self, name: &str, child: Arc<SysfsInode>) -> Result<(), FsError> {
        match &self.content {
            SysfsInodeContent::Directory(children) => {
                children.lock().insert(name.to_string(), child);
                Ok(())
            }
            _ => Err(FsError::NotDirectory),
        }
    }

    /// 读取符号链接目标
    pub fn readlink(&self) -> Result<String, FsError> {
        match &self.content {
            SysfsInodeContent::Symlink(target) => Ok(target.clone()),
            _ => Err(FsError::InvalidArgument),
        }
    }
}

impl Inode for SysfsInode {
    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        Ok(self.metadata.lock().clone())
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        match &self.content {
            SysfsInodeContent::Attribute(attr) => {
                // 调用 show 函数生成内容
                let content = (attr.show)()?;
                let data = content.as_bytes();

                if offset >= data.len() {
                    return Ok(0);
                }

                let to_read = (data.len() - offset).min(buf.len());
                buf[..to_read].copy_from_slice(&data[offset..offset + to_read]);
                Ok(to_read)
            }
            _ => Err(FsError::IsDirectory),
        }
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        match &self.content {
            SysfsInodeContent::Attribute(attr) => {
                // 检查是否支持写入
                if let Some(store) = &attr.store {
                    if offset != 0 {
                        return Err(FsError::InvalidArgument);
                    }

                    // 转换为字符串并调用 store 函数
                    let content =
                        core::str::from_utf8(buf).map_err(|_| FsError::InvalidArgument)?;

                    (store)(content)?;
                    Ok(buf.len())
                } else {
                    Err(FsError::PermissionDenied)
                }
            }
            _ => Err(FsError::IsDirectory),
        }
    }

    fn readdir(&self) -> Result<Vec<DirEntry>, FsError> {
        match &self.content {
            SysfsInodeContent::Directory(children) => {
                let mut entries = Vec::new();

                entries.push(DirEntry {
                    name: ".".to_string(),
                    inode_no: self.inode_no,
                    inode_type: InodeType::Directory,
                });

                entries.push(DirEntry {
                    name: "..".to_string(),
                    inode_no: self.inode_no,
                    inode_type: InodeType::Directory,
                });

                for (name, child) in children.lock().iter() {
                    entries.push(DirEntry {
                        name: name.clone(),
                        inode_no: child.inode_no,
                        inode_type: child.inode_type,
                    });
                }

                Ok(entries)
            }
            _ => Err(FsError::NotDirectory),
        }
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, FsError> {
        match &self.content {
            SysfsInodeContent::Directory(children) => children
                .lock()
                .get(name)
                .cloned()
                .map(|inode| inode as Arc<dyn Inode>)
                .ok_or(FsError::NotFound),
            _ => Err(FsError::NotDirectory),
        }
    }

    fn create(&self, _name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        // sysfs 是只读文件系统
        Err(FsError::ReadOnlyFs)
    }

    fn mkdir(&self, _name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        // sysfs 是只读文件系统
        Err(FsError::ReadOnlyFs)
    }

    fn truncate(&self, _size: usize) -> Result<(), FsError> {
        // sysfs 是只读文件系统
        Err(FsError::ReadOnlyFs)
    }

    fn sync(&self) -> Result<(), FsError> {
        // sysfs 是纯虚拟文件系统,无需同步
        Ok(())
    }

    fn symlink(&self, _name: &str, _target: &str) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn link(&self, _name: &str, _target: &Arc<dyn Inode>) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn unlink(&self, _name: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn rmdir(&self, _name: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn rename(
        &self,
        _old_name: &str,
        _new_parent: Arc<dyn Inode>,
        _new_name: &str,
    ) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn set_times(&self, _atime: Option<TimeSpec>, _mtime: Option<TimeSpec>) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn readlink(&self) -> Result<String, FsError> {
        match &self.content {
            SysfsInodeContent::Symlink(target) => Ok(target.clone()),
            _ => Err(FsError::InvalidArgument),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn mknod(&self, _name: &str, _mode: FileMode, _dev: u64) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotSupported)
    }

    fn chmod(&self, _mode: FileMode) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn chown(&self, _uid: u32, _gid: u32) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }
}
