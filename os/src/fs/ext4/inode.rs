//! Ext4 Inode 包装
//!
//! 将 ext4_rs 的 inode 操作包装为 VFS Inode trait
//!
//! 设计要点：
//! - 使用 Dentry 引用而非存储路径，消除与 VFS 的冗余
//! - 需要路径时动态从 Dentry.full_path() 获取

use crate::sync::SpinLock;
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use ext4_rs::InodeFileType;

use crate::vfs::{Dentry, DirEntry, FileMode, FsError, Inode, InodeMetadata, InodeType, TimeSpec};

/// Ext4 Inode 包装
pub struct Ext4Inode {
    /// ext4_rs 文件系统对象
    fs: Arc<SpinLock<ext4_rs::Ext4>>,

    /// Inode 号
    ino: u32,

    /// 关联的 Dentry（弱引用，避免循环引用）
    /// 用于获取完整路径，而不是在 Inode 中重复存储
    dentry: SpinLock<Weak<Dentry>>,
}

impl Ext4Inode {
    /// 创建新的 Ext4Inode
    ///
    /// 注意：初始创建时 dentry 为空，VFS 会在创建 Dentry 后调用 set_dentry()
    pub fn new(fs: Arc<SpinLock<ext4_rs::Ext4>>, ino: u32) -> Self {
        Self {
            fs,
            ino,
            dentry: SpinLock::new(Weak::new()),
        }
    }

    /// 辅助方法：获取完整路径（从 Dentry 动态获取）
    fn get_full_path(&self) -> Result<String, FsError> {
        let dentry = self.dentry.lock().upgrade().ok_or(FsError::IoError)?;
        Ok(dentry.full_path())
    }

    /// 辅助方法：将 ext4_rs 的 InodeFileType 转换为 VFS InodeType
    fn convert_inode_type(ft: ext4_rs::InodeFileType) -> InodeType {
        use ext4_rs::InodeFileType;
        // InodeFileType 是 bitflags，需要比较 bits()
        match ft {
            InodeFileType::S_IFREG => InodeType::File,
            InodeFileType::S_IFDIR => InodeType::Directory,
            InodeFileType::S_IFLNK => InodeType::Symlink,
            InodeFileType::S_IFCHR => InodeType::CharDevice,
            InodeFileType::S_IFBLK => InodeType::BlockDevice,
            InodeFileType::S_IFIFO => InodeType::Fifo,
            InodeFileType::S_IFSOCK => InodeType::Socket,
            _ => InodeType::File, // 默认为普通文件
        }
    }

    /// 辅助方法：将ext4_rs的DirEntryType转换为VFS InodeType
    fn convert_dir_entry_type(dentry_type: u8) -> InodeType {
        match dentry_type {
            1 => InodeType::File,        // EXT4_DE_REG_FILE
            2 => InodeType::Directory,   // EXT4_DE_DIR
            3 => InodeType::CharDevice,  // EXT4_DE_CHRDEV
            4 => InodeType::BlockDevice, // EXT4_DE_BLKDEV
            5 => InodeType::Fifo,        // EXT4_DE_FIFO
            6 => InodeType::Socket,      // EXT4_DE_SOCK
            7 => InodeType::Symlink,     // EXT4_DE_SYMLINK
            _ => InodeType::File,
        }
    }
}

impl Inode for Ext4Inode {
    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        let fs = self.fs.lock();
        let inode_ref = fs.get_inode_ref(self.ino);
        let inode = &inode_ref.inode;

        // 计算文件大小（64位）
        let size = (inode.size as u64) | ((inode.size_hi as u64) << 32);

        // 提取 inode 类型和权限
        let mode = inode.mode;
        let file_type = (mode & 0xF000) >> 12;
        let inode_type = match file_type {
            0x8 => InodeType::File,
            0x4 => InodeType::Directory,
            0xA => InodeType::Symlink,
            0x2 => InodeType::CharDevice,
            0x6 => InodeType::BlockDevice,
            0x1 => InodeType::Fifo,
            0xC => InodeType::Socket,
            _ => InodeType::File,
        };

        Ok(InodeMetadata {
            inode_no: self.ino as usize,
            size: size as usize,
            blocks: inode.blocks as usize,
            atime: TimeSpec {
                sec: inode.atime as i64,
                nsec: 0,
            },
            mtime: TimeSpec {
                sec: inode.mtime as i64,
                nsec: 0,
            },
            ctime: TimeSpec {
                sec: inode.ctime as i64,
                nsec: 0,
            },
            inode_type,
            mode: FileMode::from_bits_truncate(mode as u32),
            nlinks: inode.links_count as usize,
            uid: inode.uid as u32,
            gid: inode.gid as u32,
        })
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        // Check if this is a directory
        let metadata = self.metadata()?;
        if metadata.inode_type == InodeType::Directory {
            return Err(FsError::IsDirectory);
        }

        let fs = self.fs.lock();

        // ext4_rs 的 read_at 签名: pub fn read_at(&self, inode: u32, offset: usize, read_buf: &mut [u8])
        fs.read_at(self.ino, offset, buf)
            .map_err(|_| FsError::IoError)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        // Check if this is a directory
        let metadata = self.metadata()?;
        if metadata.inode_type == InodeType::Directory {
            return Err(FsError::IsDirectory);
        }

        let fs = self.fs.lock();

        // ext4_rs 的 write_at 签名: pub fn write_at(&self, inode: u32, offset: usize, write_buf: &[u8])
        fs.write_at(self.ino, offset, buf)
            .map_err(|_| FsError::IoError)
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, FsError> {
        // Check if current inode is a directory
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        // 类似 create,lookup 也应该使用相对路径
        // 直接在当前目录下查找指定名称的文件
        let mut fs = self.fs.lock();
        let mut parent = self.ino;
        let mut name_off = 0;

        // 直接使用文件名作为路径
        let child_ino = fs
            .generic_open(name, &mut parent, false, 0, &mut name_off)
            .map_err(|_| FsError::NotFound)?;

        // 创建子 Inode（暂时没有 dentry，VFS 会调用 set_dentry）
        Ok(Arc::new(Ext4Inode::new(self.fs.clone(), child_ino)))
    }

    fn create(&self, name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        // Check if current inode is a directory
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        // Check if file already exists
        if self.lookup(name).is_ok() {
            return Err(FsError::AlreadyExists);
        }

        // create() only creates regular files (not directories)
        let fs = self.fs.lock();
        // ext4_rs 的 create_inode 内部会强制设置 mode |= 0o777
        // 所以这里直接使用 0o777
        let ftype = ext4_rs::InodeFileType::S_IFREG.bits() | 0o777;

        let child_inode = fs
            .create(self.ino, name, ftype)
            .map_err(|_| FsError::IoError)?;

        Ok(Arc::new(Ext4Inode::new(
            self.fs.clone(),
            child_inode.inode_num,
        )))
    }

    fn mkdir(&self, name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        // Check if current inode is a directory
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        // Check if directory already exists
        if self.lookup(name).is_ok() {
            return Err(FsError::AlreadyExists);
        }

        // mkdir() creates directories using S_IFDIR | 0o755
        let fs = self.fs.lock();
        let ftype = ext4_rs::InodeFileType::S_IFDIR.bits() | 0o755;

        let mut parent = self.ino;
        let mut name_off = 0;

        let inode_id = fs
            .generic_open(name, &mut parent, true, ftype, &mut name_off)
            .map_err(|e| {
                crate::println!("[Ext4Inode::mkdir] generic_open failed: {:?}", e);
                FsError::NoSpace
            })?;

        Ok(Arc::new(Ext4Inode::new(self.fs.clone(), inode_id)))
    }

    fn symlink(&self, name: &str, target: &str) -> Result<Arc<dyn Inode>, FsError> {
        // Check if current inode is a directory
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        let parent = self.ino;
        let inode_mod = InodeFileType::S_IFLNK.bits() | 0o777;
        let fs = self.fs.lock();

        let new_inode = fs
            .create(parent, name, inode_mod)
            .map_err(|_| FsError::NoSpace)?;

        fs.write_at(new_inode.inode_num, 0, target.as_bytes())
            .map_err(|_| FsError::IoError)?;

        Ok(Arc::new(Ext4Inode::new(
            self.fs.clone(),
            new_inode.inode_num,
        )))
    }

    fn link(&self, name: &str, target: &Arc<dyn Inode>) -> Result<(), FsError> {
        // Check if current inode is a directory
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        // 向下转型为 Ext4Inode 以获取 inode 号
        let ext4_inode = target
            .downcast_ref::<Ext4Inode>()
            .ok_or(FsError::InvalidArgument)?;

        if !Arc::ptr_eq(&self.fs, &ext4_inode.fs) {
            return Err(FsError::InvalidArgument);
        }

        let fs = self.fs.lock();
        let mut self_ref = fs.get_inode_ref(self.ino);
        let mut target_ref = fs.get_inode_ref(ext4_inode.ino);
        fs.link(&mut self_ref, &mut target_ref, name)
            .map_err(|_| FsError::NoSpace)?;

        Ok(())
    }

    fn unlink(&self, name: &str) -> Result<(), FsError> {
        // Check if current inode is a directory
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        // 查找要删除的项
        let child = self.lookup(name)?;
        let child_metadata = child.metadata()?;

        // 获取 child 的 inode 号
        let child_ext4 = child
            .as_any()
            .downcast_ref::<Ext4Inode>()
            .ok_or(FsError::InvalidArgument)?;

        let fs = self.fs.lock();

        // Workaround for ext4_rs bug: dir_remove() 无条件调用 dir_has_entry()
        // 但 dir_has_entry() 内部 assert child 必须是目录
        // 所以对于普通文件，我们需要使用底层的 API 绕过这个 bug

        if child_metadata.inode_type == InodeType::Directory {
            // 对于目录，使用 dir_remove（它会检查目录是否为空）
            fs.dir_remove(self.ino, name)
                .map_err(|_| FsError::IoError)?;
        } else {
            // 对于普通文件，使用底层 API 手动删除
            let mut parent_ref = fs.get_inode_ref(self.ino);
            let mut child_ref = fs.get_inode_ref(child_ext4.ino);

            // 调用底层的 unlink，它会：
            // 1. 删除目录项（dir_remove_entry）
            // 2. 释放 inode（ialloc_free_inode）
            fs.unlink(&mut parent_ref, &mut child_ref, name)
                .map_err(|_| FsError::IoError)?;

            // 写回 parent inode
            fs.write_back_inode(&mut parent_ref);
        }

        Ok(())
    }

    fn rmdir(&self, name: &str) -> Result<(), FsError> {
        // Check if current inode is a directory
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        let fs = self.fs.lock();
        let parent = self.ino;

        fs.dir_remove(parent, name)
            .map(|_| ())
            .map_err(|_| FsError::NotFound)
    }

    fn rename(
        &self,
        old_name: &str,
        new_parent: Arc<dyn Inode>,
        new_name: &str,
    ) -> Result<(), FsError> {
        todo!("implement rename");
    }

    fn readdir(&self) -> Result<Vec<DirEntry>, FsError> {
        // Check if current inode is a directory
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        let fs = self.fs.lock();

        // ext4_rs 的 dir_get_entries 签名: pub fn dir_get_entries(&self, inode: u32) -> Vec<Ext4DirEntry>
        // 直接返回 Vec，不需要 map_err
        let entries = fs.dir_get_entries(self.ino);

        // 转换为 VFS 的 DirEntry 格式
        let vfs_entries = entries
            .iter()
            .map(|e| {
                // Ext4DirEntry 的 name 字段是 [u8; 255]，需要转换为 String
                let name_len = e.name_len as usize;
                let name = String::from_utf8_lossy(&e.name[..name_len]).into_owned();

                // inner 是 union，需要 unsafe 访问 inode_type 字段
                let inode_type = unsafe { Self::convert_dir_entry_type(e.inner.inode_type) };

                DirEntry {
                    name,
                    inode_type,
                    inode_no: e.inode as usize,
                }
            })
            .collect();

        Ok(vfs_entries)
    }

    fn truncate(&self, size: usize) -> Result<(), FsError> {
        let metadata = self.metadata()?;
        let old_size = metadata.size;

        if size == old_size {
            // 大小不变，直接返回
            return Ok(());
        }

        if size < old_size {
            // 缩小文件：使用 ext4_rs 的 truncate_inode
            let fs = self.fs.lock();
            let mut inode_ref = fs.get_inode_ref(self.ino);
            fs.truncate_inode(&mut inode_ref, size as u64)
                .map_err(|_| FsError::IoError)?;
        } else {
            // 扩展文件：ext4_rs 的 truncate_inode 不支持扩展（有 assert）
            // Workaround: 在文件末尾写入零字节来扩展
            // 这是安全的，因为：
            // 1. 写入位置从 old_size 开始，在现有数据之后
            // 2. write_at 会分配新块并更新文件大小
            // 3. 符合 POSIX truncate 语义（新增部分填充零）
            let extend_size = size - old_size;
            let zero_buf = alloc::vec![0u8; extend_size.min(4096)]; // 使用 4KB 缓冲区

            let fs = self.fs.lock();
            let mut written = 0;
            while written < extend_size {
                let to_write = (extend_size - written).min(zero_buf.len());
                fs.write_at(self.ino, old_size + written, &zero_buf[..to_write])
                    .map_err(|_| FsError::IoError)?;
                written += to_write;
            }
        }

        Ok(())
    }

    fn sync(&self) -> Result<(), FsError> {
        // ext4_rs 会自动同步数据到 BlockDevice
        Ok(())
    }

    fn set_dentry(&self, dentry: Weak<Dentry>) {
        *self.dentry.lock() = dentry;
    }

    fn get_dentry(&self) -> Option<Arc<Dentry>> {
        self.dentry.lock().upgrade()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
