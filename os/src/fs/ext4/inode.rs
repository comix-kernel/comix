//! Ext4 Inode 包装
//!
//! 将 ext4_rs 的 inode 操作包装为 VFS Inode trait
//!
//! 设计要点：
//! - 使用 Dentry 引用而非存储路径，消除与 VFS 的冗余
//! - 需要路径时动态从 Dentry.full_path() 获取

use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use crate::sync::SpinLock;

use crate::vfs::{Dentry, Inode, InodeType, DirEntry, FsError, InodeMetadata, FileMode, TimeSpec};

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
        let dentry = self.dentry.lock()
            .upgrade()
            .ok_or(FsError::IoError)?;
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
            _ => InodeType::File,  // 默认为普通文件
        }
    }

    /// 辅助方法：将ext4_rs的DirEntryType转换为VFS InodeType
    fn convert_dir_entry_type(dentry_type: u8) -> InodeType {
        match dentry_type {
            1 => InodeType::File,         // EXT4_DE_REG_FILE
            2 => InodeType::Directory,    // EXT4_DE_DIR
            3 => InodeType::CharDevice,   // EXT4_DE_CHRDEV
            4 => InodeType::BlockDevice,  // EXT4_DE_BLKDEV
            5 => InodeType::Fifo,         // EXT4_DE_FIFO
            6 => InodeType::Socket,       // EXT4_DE_SOCK
            7 => InodeType::Symlink,      // EXT4_DE_SYMLINK
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
            atime: TimeSpec { sec: inode.atime as i64, nsec: 0 },
            mtime: TimeSpec { sec: inode.mtime as i64, nsec: 0 },
            ctime: TimeSpec { sec: inode.ctime as i64, nsec: 0 },
            inode_type,
            mode: FileMode::from_bits_truncate(mode as u32),
            nlinks: inode.links_count as usize,
            uid: inode.uid as u32,
            gid: inode.gid as u32,
        })
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        let fs = self.fs.lock();

        // ext4_rs 的 read_at 签名: pub fn read_at(&self, inode: u32, offset: usize, read_buf: &mut [u8])
        fs.read_at(self.ino, offset, buf)
            .map_err(|_| FsError::IoError)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        let fs = self.fs.lock();

        // ext4_rs 的 write_at 签名: pub fn write_at(&self, inode: u32, offset: usize, write_buf: &[u8])
        fs.write_at(self.ino, offset, buf)
            .map_err(|_| FsError::IoError)
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, FsError> {
        // 从 Dentry 动态获取完整路径
        let current_path = self.get_full_path()?;
        let full_path = if current_path == "/" {
            alloc::format!("/{}", name)
        } else {
            alloc::format!("{}/{}", current_path, name)
        };

        // ext4_rs 的 generic_open 签名:
        // pub fn generic_open(&self, path: &str, parent: &mut u32, create: bool, ftype: u16, name_off: &mut u32) -> Result<u32>
        let mut fs = self.fs.lock();
        let mut parent = self.ino;
        let mut name_off = 0;

        let child_ino = fs.generic_open(&full_path, &mut parent, false, 0, &mut name_off)
            .map_err(|_| FsError::NotFound)?;

        // 创建子 Inode（暂时没有 dentry，VFS 会调用 set_dentry）
        Ok(Arc::new(Ext4Inode::new(self.fs.clone(), child_ino)))
    }

    fn create(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        let current_path = self.get_full_path()?;
        let full_path = if current_path == "/" {
            alloc::format!("/{}", name)
        } else {
            alloc::format!("{}/{}", current_path, name)
        };

        let fs = self.fs.lock();

        // 从 FileMode 提取文件类型位（高4位）
        let file_type_bits = mode.bits() & 0o170000;

        // 将 FileMode 的文件类型转换为 ext4_rs 的 ftype
        let ftype = if file_type_bits == FileMode::S_IFDIR.bits() {
            ext4_rs::InodeFileType::S_IFDIR.bits()
        } else if file_type_bits == FileMode::S_IFLNK.bits() {
            ext4_rs::InodeFileType::S_IFLNK.bits()
        } else {
            ext4_rs::InodeFileType::S_IFREG.bits()  // 默认为普通文件
        };

        // generic_open 的签名: (path, parent, create, ftype, name_off)
        let mut parent = self.ino;
        let mut name_off = 0;

        let child_ino = fs.generic_open(&full_path, &mut parent, true, ftype, &mut name_off)
            .map_err(|_| FsError::AlreadyExists)?;

        Ok(Arc::new(Ext4Inode::new(self.fs.clone(), child_ino)))
    }

    fn mkdir(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        // mkdir 就是创建目录类型的文件
        // 确保 mode 包含目录类型位
        let dir_mode = FileMode::S_IFDIR | (mode & !FileMode::S_IFMT);
        self.create(name, dir_mode)
    }

    fn unlink(&self, name: &str) -> Result<(), FsError> {
        let fs = self.fs.lock();
        // ext4_rs 的 dir_remove 签名: pub fn dir_remove(&self, parent: u32, path: &str) -> Result<usize>
        // unlink 删除文件或目录，都使用 dir_remove
        fs.dir_remove(self.ino, name)
            .map_err(|_| FsError::IoError)?;

        Ok(())
    }

    fn readdir(&self) -> Result<Vec<DirEntry>, FsError> {
        let fs = self.fs.lock();

        // ext4_rs 的 dir_get_entries 签名: pub fn dir_get_entries(&self, inode: u32) -> Vec<Ext4DirEntry>
        // 直接返回 Vec，不需要 map_err
        let entries = fs.dir_get_entries(self.ino);

        // 转换为 VFS 的 DirEntry 格式
        let vfs_entries = entries.iter().map(|e| {
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
        }).collect();

        Ok(vfs_entries)
    }

    fn truncate(&self, size: usize) -> Result<(), FsError> {
        let fs = self.fs.lock();
        let mut inode_ref = fs.get_inode_ref(self.ino);

        // ext4_rs 的 truncate_inode 签名: pub fn truncate_inode(&self, inode_ref: &mut Ext4InodeRef, new_size: u64) -> Result<usize>
        fs.truncate_inode(&mut inode_ref, size as u64)
            .map_err(|_| FsError::IoError)?;

        Ok(())
    }

    fn sync(&self) -> Result<(), FsError> {
        // ext4_rs 会自动同步数据到 BlockDevice
        // 这里我们只需要 flush 底层设备即可
        // 如果需要强制写回 inode，可以调用 write_back_inode
        Ok(())
    }

    fn set_dentry(&self, dentry: Weak<Dentry>) {
        *self.dentry.lock() = dentry;
    }

    fn get_dentry(&self) -> Option<Arc<Dentry>> {
        self.dentry.lock().upgrade()
    }
}