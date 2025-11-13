//! 简单的文件系统实现（用于测试/调试）

use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::collections::BTreeMap;
use crate::sync::SpinLock;
use crate::vfs::*;

/// 简单的内存文件系统（用于测试）
pub struct SimpleFs {
    root: Arc<SimpleFsInode>,
}

impl SimpleFs {
    /// 创建新的简单文件系统
    pub fn new() -> Arc<Self> {
        let root = SimpleFsInode::new_dir(1);
        Arc::new(Self { root })
    }
}

impl FileSystem for SimpleFs {
    fn fs_type(&self) -> &'static str {
        "simplefs"
    }

    fn root_inode(&self) -> Arc<dyn Inode> {
        self.root.clone() as Arc<dyn Inode>
    }

    fn sync(&self) -> Result<(), FsError> {
        Ok(()) // 内存文件系统无需同步
    }

    fn statfs(&self) -> Result<StatFs, FsError> {
        Ok(StatFs {
            block_size: 4096,
            total_blocks: 0,
            free_blocks: 0,
            available_blocks: 0,
            total_inodes: 0,
            free_inodes: 0,
            fsid: 0,
            max_filename_len: 255,
        })
    }
}

/// 简单文件系统的 Inode
struct SimpleFsInode {
    inode_no: usize,
    inode_type: InodeType,
    mode: FileMode,
    data: SpinLock<Vec<u8>>,
    children: SpinLock<BTreeMap<String, Arc<SimpleFsInode>>>,
}

impl SimpleFsInode {
    /// 创建文件 inode
    fn new_file(inode_no: usize) -> Arc<Self> {
        Arc::new(Self {
            inode_no,
            inode_type: InodeType::File,
            mode: FileMode::S_IFREG | FileMode::S_IRUSR | FileMode::S_IWUSR,
            data: SpinLock::new(Vec::new()),
            children: SpinLock::new(BTreeMap::new()),
        })
    }

    /// 创建目录 inode
    fn new_dir(inode_no: usize) -> Arc<Self> {
        Arc::new(Self {
            inode_no,
            inode_type: InodeType::Directory,
            mode: FileMode::S_IFDIR | FileMode::S_IRUSR | FileMode::S_IWUSR | FileMode::S_IXUSR,
            data: SpinLock::new(Vec::new()),
            children: SpinLock::new(BTreeMap::new()),
        })
    }
}

impl Inode for SimpleFsInode {
    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        let data = self.data.lock();
        Ok(InodeMetadata {
            inode_no: self.inode_no,
            inode_type: self.inode_type,
            mode: self.mode,
            uid: 0,
            gid: 0,
            size: data.len(),
            atime: TimeSpec::now(),
            mtime: TimeSpec::now(),
            ctime: TimeSpec::now(),
            nlinks: 1,
            blocks: (data.len() + 511) / 512,
        })
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        let data = self.data.lock();
        if offset >= data.len() {
            return Ok(0);
        }
        let len = core::cmp::min(buf.len(), data.len() - offset);
        buf[..len].copy_from_slice(&data[offset..offset + len]);
        Ok(len)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        let mut data = self.data.lock();
        if offset + buf.len() > data.len() {
            data.resize(offset + buf.len(), 0);
        }
        data[offset..offset + buf.len()].copy_from_slice(buf);
        Ok(buf.len())
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, FsError> {
        if self.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        self.children
            .lock()
            .get(name)
            .cloned()
            .ok_or(FsError::NotFound)
            .map(|inode| inode as Arc<dyn Inode>)
    }

    fn create(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        if self.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        let mut children = self.children.lock();
        if children.contains_key(name) {
            return Err(FsError::AlreadyExists);
        }

        let new_inode = SimpleFsInode::new_file(children.len() + 2);
        children.insert(String::from(name), new_inode.clone());

        Ok(new_inode as Arc<dyn Inode>)
    }

    fn mkdir(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        if self.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        let mut children = self.children.lock();
        if children.contains_key(name) {
            return Err(FsError::AlreadyExists);
        }

        let new_inode = SimpleFsInode::new_dir(children.len() + 2);
        children.insert(String::from(name), new_inode.clone());

        Ok(new_inode as Arc<dyn Inode>)
    }

    fn unlink(&self, name: &str) -> Result<(), FsError> {
        if self.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        let mut children = self.children.lock();
        children.remove(name).ok_or(FsError::NotFound)?;
        Ok(())
    }

    fn readdir(&self) -> Result<Vec<DirEntry>, FsError> {
        if self.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        let children = self.children.lock();
        let mut entries = Vec::new();

        for (name, child) in children.iter() {
            entries.push(DirEntry {
                name: name.clone(),
                inode_no: child.inode_no,
                inode_type: child.inode_type,
            });
        }

        Ok(entries)
    }

    fn truncate(&self, size: usize) -> Result<(), FsError> {
        let mut data = self.data.lock();
        data.resize(size, 0);
        Ok(())
    }

    fn sync(&self) -> Result<(), FsError> {
        Ok(())
    }
}