//! SimpleFS - 简单测试文件系统
//!
//! 该模块提供了一个**轻量级的只读文件系统**，用于测试和调试。镜像在编译时嵌入内核。
//!
//! # 设计概览
//!
//! ## 镜像格式
//!
//! ```text
//! +------------------+
//! | Header (512B)    |  Magic: "RAMDISK\0", File count
//! +------------------+
//! | File Entry 1     |  Header (32B) + Name + Data
//! +------------------+
//! | File Entry 2     |
//! | ...              |
//! +------------------+
//! ```
//!
//! ## 加载流程
//!
//! 1. 编译时 `build.rs` 生成镜像并嵌入
//! 2. 启动时从 `include_bytes!` 加载到 RamDisk
//! 3. 解析镜像构建目录树
//!
//! # 组件
//!
//! - [`SimpleFs`] - 文件系统结构，实现 `FileSystem` trait
//! - `SimpleFsInode` - 内部 Inode 实现
//!
//! # 使用示例
//!
//! ```rust
//! use crate::fs::init_simple_fs;
//!
//! // 从编译时嵌入的镜像加载
//! init_simple_fs()?;
//!
//! // 读取预加载的文件
//! let hello = vfs_load_file("/bin/hello")?;
//! ```
//!
//! # 特点
//!
//! - **只读**：运行时不可修改
//! - **快速启动**：无需磁盘 I/O
//! - **测试友好**：提供一致的测试环境
//! - **自动路径创建**：支持多级路径（如 `bin/hello`）

use crate::sync::SpinLock;
use crate::vfs::*;
use crate::{device::block::BlockDriver, uapi::time::TimeSpec};
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

/// 简单的内存文件系统（用于测试）
pub struct SimpleFs {
    device: Option<Arc<dyn BlockDriver>>, // 可选的块设备
    root: Arc<SimpleFsInode>,
}

impl SimpleFs {
    /// 创建新的简单文件系统
    pub fn new() -> Arc<Self> {
        let root = Arc::new(SimpleFsInode::new_dir(
            1,
            FileMode::S_IRUSR
                | FileMode::S_IWUSR
                | FileMode::S_IXUSR
                | FileMode::S_IRGRP
                | FileMode::S_IXGRP
                | FileMode::S_IROTH
                | FileMode::S_IXOTH,
        ));
        Arc::new(Self { device: None, root })
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

impl SimpleFs {
    /// 从块设备和镜像创建 SimpleFS
    pub fn from_ramdisk(device: Arc<dyn BlockDriver>) -> Result<Self, FsError> {
        // 1. 读取镜像头，验证魔数
        let block_size = device.block_size();
        let mut header_block = vec![0u8; block_size];
        if !device.read_block(0, &mut header_block) {
            return Err(FsError::IoError);
        }

        if &header_block[0..8] != b"RAMDISK\0" {
            return Err(FsError::IoError);
        }

        let file_count = u32::from_le_bytes(header_block[8..12].try_into().unwrap());

        // 2. 创建根目录 inode (0o755 = rwxr-xr-x)
        let root = Arc::new(SimpleFsInode::new_dir(
            1,
            FileMode::S_IRUSR
                | FileMode::S_IWUSR
                | FileMode::S_IXUSR
                | FileMode::S_IRGRP
                | FileMode::S_IXGRP
                | FileMode::S_IROTH
                | FileMode::S_IXOTH,
        ));

        // 3. 解析镜像，填充文件树
        let mut offset = 16;
        for _ in 0..file_count {
            offset = Self::parse_file_entry(device.clone(), offset, root.clone())?;
        }

        Ok(Self {
            device: Some(device),
            root,
        })
    }

    /// 解析镜像中的单个文件条目并添加到父目录
    fn parse_file_entry(
        device: Arc<dyn BlockDriver>,
        offset: usize,
        parent: Arc<SimpleFsInode>,
    ) -> Result<usize, FsError> {
        // 读取文件头 (32字节)
        let mut header_buf = vec![0u8; 32];
        Self::read_at_offset(device.clone(), offset, &mut header_buf)?;

        let magic = u32::from_le_bytes(header_buf[0..4].try_into().unwrap());
        if magic != 0x46494C45 {
            return Err(FsError::IoError);
        }

        let name_len = u32::from_le_bytes(header_buf[4..8].try_into().unwrap()) as usize;
        let data_len = u32::from_le_bytes(header_buf[8..12].try_into().unwrap()) as usize;
        let file_type = u32::from_le_bytes(header_buf[12..16].try_into().unwrap());
        let mode = u32::from_le_bytes(header_buf[16..20].try_into().unwrap());

        let mut cur_offset = offset + 32;

        // 读取文件名
        let name_aligned = (name_len + 3) / 4 * 4;
        let mut name_buf = vec![0u8; name_aligned];
        Self::read_at_offset(device.clone(), cur_offset, &mut name_buf)?;
        let name = String::from(String::from_utf8_lossy(&name_buf[..name_len]));
        cur_offset += name_aligned;

        // 读取文件数据
        let data_aligned = (data_len + 511) / 512 * 512;
        let mut data_buf = vec![0u8; data_len];
        if data_len > 0 {
            Self::read_at_offset(device.clone(), cur_offset, &mut data_buf)?;
        }
        cur_offset += data_aligned;

        // 创建 inode 并添加到父目录
        let inode = if file_type == 0 {
            // 文件 (mode as u16 转换为 FileMode)
            let file_mode = FileMode::from_bits_truncate(mode);
            let inode = SimpleFsInode::new_file(parent.next_inode_no(), file_mode);
            inode.data.lock().extend_from_slice(&data_buf);
            Arc::new(inode)
        } else {
            // 目录
            let dir_mode = FileMode::from_bits_truncate(mode);
            Arc::new(SimpleFsInode::new_dir(parent.next_inode_no(), dir_mode))
        };

        // 处理多级路径 (如 "bin/hello")
        Self::insert_inode_by_path(&name, inode, parent)?;

        Ok(cur_offset)
    }

    /// 从设备的任意偏移位置读取数据（支持跨块读取）
    fn read_at_offset(
        device: Arc<dyn BlockDriver>,
        offset: usize,
        buf: &mut [u8],
    ) -> Result<(), FsError> {
        let block_size = device.block_size();
        let mut buf_offset = 0;

        while buf_offset < buf.len() {
            // 计算当前应该读取的块号和块内偏移
            let current_offset = offset + buf_offset;
            let block_num = current_offset / block_size;
            let block_offset = current_offset % block_size;

            let mut block_buf = vec![0u8; block_size];
            if !device.read_block(block_num, &mut block_buf) {
                return Err(FsError::IoError);
            }

            // 计算本次要复制的字节数
            let copy_len = (block_size - block_offset).min(buf.len() - buf_offset);

            buf[buf_offset..buf_offset + copy_len]
                .copy_from_slice(&block_buf[block_offset..block_offset + copy_len]);

            buf_offset += copy_len;
        }

        Ok(())
    }

    /// 按路径插入 inode，自动创建不存在的中间目录
    fn insert_inode_by_path(
        path: &str,
        inode: Arc<SimpleFsInode>,
        root: Arc<SimpleFsInode>,
    ) -> Result<(), FsError> {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        if parts.is_empty() {
            return Err(FsError::InvalidArgument);
        }

        let mut current = root;

        // 遍历到倒数第二级，确保父目录存在
        for part in &parts[..parts.len() - 1] {
            let child = {
                let children = current.children.lock();
                children.get(*part).cloned()
            };

            if let Some(child) = child {
                current = child;
            } else {
                // 创建中间目录 (0o755 = rwxr-xr-x)
                let new_dir = Arc::new(SimpleFsInode::new_dir(
                    current.next_inode_no(),
                    FileMode::S_IRUSR
                        | FileMode::S_IWUSR
                        | FileMode::S_IXUSR
                        | FileMode::S_IRGRP
                        | FileMode::S_IXGRP
                        | FileMode::S_IROTH
                        | FileMode::S_IXOTH,
                ));
                current
                    .children
                    .lock()
                    .insert(String::from(*part), new_dir.clone());
                current = new_dir;
            }
        }

        // 插入最终的文件/目录
        let final_name = parts[parts.len() - 1];
        current
            .children
            .lock()
            .insert(String::from(final_name), inode);

        Ok(())
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
    /// 创建文件 inode（带权限）
    fn new_file(inode_no: u64, mode: FileMode) -> Self {
        let file_mode = FileMode::S_IFREG | mode;

        Self {
            inode_no: inode_no as usize,
            inode_type: InodeType::File,
            mode: file_mode,
            data: SpinLock::new(Vec::new()),
            children: SpinLock::new(BTreeMap::new()),
        }
    }

    /// 创建目录 inode（带权限）
    fn new_dir(inode_no: u64, mode: FileMode) -> Self {
        let dir_mode = FileMode::S_IFDIR | mode;

        Self {
            inode_no: inode_no as usize,
            inode_type: InodeType::Directory,
            mode: dir_mode,
            data: SpinLock::new(Vec::new()),
            children: SpinLock::new(BTreeMap::new()),
        }
    }

    /// 获取下一个 inode 编号
    fn next_inode_no(&self) -> u64 {
        static NEXT_INODE: AtomicU64 = AtomicU64::new(2);
        NEXT_INODE.fetch_add(1, Ordering::Relaxed)
    }
}

impl Inode for SimpleFsInode {
    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        let data = self.data.lock();
        Ok(InodeMetadata {
            inode_no: self.inode_no,
            inode_type: self.inode_type,
            mode: self.mode.clone(),
            uid: 0,
            gid: 0,
            size: data.len(),
            atime: TimeSpec::now(),
            mtime: TimeSpec::now(),
            ctime: TimeSpec::now(),
            nlinks: 1,
            blocks: (data.len() + 511) / 512,
            rdev: 0,
        })
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        if self.inode_type == InodeType::Directory {
            return Err(FsError::IsDirectory);
        }

        let data = self.data.lock();
        if offset >= data.len() {
            return Ok(0);
        }
        let len = core::cmp::min(buf.len(), data.len() - offset);

        buf[..len].copy_from_slice(&data[offset..offset + len]);
        Ok(len)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        if self.inode_type == InodeType::Directory {
            return Err(FsError::IsDirectory);
        }

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

        let new_inode = Arc::new(SimpleFsInode::new_file((children.len() + 2) as u64, mode));
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

        let new_inode = Arc::new(SimpleFsInode::new_dir((children.len() + 2) as u64, mode));
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

    fn as_any(&self) -> &dyn core::any::Any {
        self as &dyn core::any::Any
    }

    fn symlink(&self, _name: &str, _target: &str) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotSupported)
    }

    fn link(&self, _name: &str, _target: &Arc<dyn Inode>) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn rmdir(&self, _name: &str) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn rename(
        &self,
        _old_name: &str,
        _new_parent: Arc<dyn Inode>,
        _new_name: &str,
    ) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn set_times(&self, _atime: Option<TimeSpec>, _mtime: Option<TimeSpec>) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn readlink(&self) -> Result<String, FsError> {
        Err(FsError::NotSupported)
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
