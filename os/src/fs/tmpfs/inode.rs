//! Tmpfs Inode 实现
//!
//! TmpfsInode 直接管理物理帧，无需经过 BlockDevice 层

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;

use crate::config::PAGE_SIZE;
use crate::mm::address::{ConvertablePaddr, PageNum, UsizeConvert};
use crate::mm::frame_allocator::{FrameTracker, alloc_frame};
use crate::sync::{Mutex, SpinLock};
use crate::uapi::time::TimeSpec;
use crate::vfs::{DirEntry, FileMode, FsError, Inode, InodeMetadata, InodeType};

/// Tmpfs Inode 实现
///
/// 文件数据直接存储在物理页中，按需分配
pub struct TmpfsInode {
    /// Inode 元数据
    metadata: SpinLock<InodeMetadata>,

    /// 文件数据页（稀疏存储）
    /// 索引：页号 (offset / PAGE_SIZE)
    /// 值：物理帧 (None 表示空洞)
    data: Mutex<Vec<Option<Arc<FrameTracker>>>>,

    /// 父目录（弱引用，避免循环引用）
    parent: Mutex<Weak<TmpfsInode>>,

    /// 子节点（仅对目录有效）
    children: Mutex<BTreeMap<String, Arc<TmpfsInode>>>,

    /// Tmpfs 统计信息（共享引用）
    stats: Arc<Mutex<TmpfsStats>>,

    /// 指向自身的弱引用（用于 lookup "." 和作为子节点的父节点）
    self_ref: Mutex<Weak<TmpfsInode>>,
}

/// Tmpfs 统计信息
#[derive(Debug, Clone)]
pub struct TmpfsStats {
    /// 已分配的总页数
    pub allocated_pages: usize,

    /// 最大允许的页数（0 表示无限制）
    pub max_pages: usize,

    /// 下一个 inode 编号
    pub next_inode_no: usize,
}

impl TmpfsInode {
    /// 创建新的 tmpfs inode（通用构造函数）
    pub fn new(
        inode_no: usize,
        inode_type: InodeType,
        mode: FileMode,
        parent: Weak<TmpfsInode>,
        stats: Arc<Mutex<TmpfsStats>>,
    ) -> Arc<Self> {
        let now = TimeSpec::now();

        // 清除文件类型位，只保留权限位和特殊位
        let mode = mode & !FileMode::S_IFMT;

        // 根据 inode_type 设置正确的文件类型位
        let mode = match inode_type {
            InodeType::Directory => mode | FileMode::S_IFDIR,
            InodeType::File => mode | FileMode::S_IFREG,
            InodeType::Symlink => mode | FileMode::S_IFLNK,
            InodeType::CharDevice => mode | FileMode::S_IFCHR,
            InodeType::BlockDevice => mode | FileMode::S_IFBLK,
            InodeType::Fifo => mode | FileMode::S_IFIFO,
            InodeType::Socket => mode | FileMode::S_IFSOCK,
        };

        let metadata = InodeMetadata {
            inode_no,
            inode_type,
            mode,
            uid: 0,
            gid: 0,
            size: 0,
            atime: now,
            mtime: now,
            ctime: now,
            nlinks: if inode_type == InodeType::Directory {
                2
            } else {
                1
            }, // 目录默认2（.和..）
            blocks: 0,
            rdev: 0, // 设备节点会在 mknod 时设置
        };

        Arc::new(Self {
            metadata: SpinLock::new(metadata),
            data: Mutex::new(Vec::new()),
            parent: Mutex::new(parent),
            children: Mutex::new(BTreeMap::new()),
            stats,
            self_ref: Mutex::new(Weak::new()),
        })
    }

    /// 创建根目录
    pub fn new_root(stats: Arc<Mutex<TmpfsStats>>) -> Arc<Self> {
        let stats_guard = stats.lock();
        let inode_no = stats_guard.next_inode_no;
        drop(stats_guard);

        let mut stats_guard = stats.lock();
        stats_guard.next_inode_no += 1;
        drop(stats_guard);

        let root = Self::new(
            inode_no,
            InodeType::Directory,
            FileMode::S_IRUSR
                | FileMode::S_IWUSR
                | FileMode::S_IXUSR
                | FileMode::S_IRGRP
                | FileMode::S_IXGRP
                | FileMode::S_IROTH
                | FileMode::S_IXOTH,
            Weak::new(),
            stats,
        );

        // 设置自引用
        *root.self_ref.lock() = Arc::downgrade(&root);

        root
    }

    /// 分配新的 inode 编号
    fn alloc_inode_no(&self) -> usize {
        let mut stats = self.stats.lock();
        let inode_no = stats.next_inode_no;
        stats.next_inode_no += 1;
        inode_no
    }

    /// 检查是否有足够的空间分配新页
    fn can_alloc_pages(&self, num_pages: usize) -> bool {
        let stats = self.stats.lock();
        if stats.max_pages == 0 {
            return true; // 无限制
        }
        stats.allocated_pages + num_pages <= stats.max_pages
    }

    /// 增加已分配页数
    fn inc_allocated_pages(&self, num: usize) {
        let mut stats = self.stats.lock();
        stats.allocated_pages += num;
    }

    /// 减少已分配页数
    fn dec_allocated_pages(&self, num: usize) {
        let mut stats = self.stats.lock();
        stats.allocated_pages = stats.allocated_pages.saturating_sub(num);
    }

    /// 更新访问时间
    fn update_atime(&self) {
        let mut meta = self.metadata.lock();
        meta.atime = TimeSpec::now();
    }

    /// 更新修改时间
    fn update_mtime(&self) {
        let mut meta = self.metadata.lock();
        let now = TimeSpec::now();
        meta.mtime = now;
        meta.ctime = now;
    }

    fn reserve_page(&self) -> Result<(), FsError> {
        let mut stats = self.stats.lock();
        if stats.max_pages != 0 && stats.allocated_pages >= stats.max_pages {
            return Err(FsError::NoSpace);
        }
        stats.allocated_pages += 1;
        Ok(())
    }

    fn cancel_page_reservation(&self) {
        self.dec_allocated_pages(1);
    }
}

impl Inode for TmpfsInode {
    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        Ok(self.metadata.lock().clone())
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        let meta = self.metadata.lock();

        if meta.inode_type != InodeType::File {
            return Err(FsError::IsDirectory);
        }

        if offset >= meta.size {
            return Ok(0);
        }

        let read_size = buf.len().min(meta.size - offset);
        drop(meta);

        let mut bytes_read = 0;
        let data = self.data.lock();

        while bytes_read < read_size {
            let page_index = (offset + bytes_read) / PAGE_SIZE;
            let page_offset = (offset + bytes_read) % PAGE_SIZE;
            let read_len = (PAGE_SIZE - page_offset).min(read_size - bytes_read);

            // 如果页不存在，返回 0
            if page_index >= data.len() || data[page_index].is_none() {
                buf[bytes_read..bytes_read + read_len].fill(0);
            } else {
                // 通过内核直接映射读取
                let frame = data[page_index].as_ref().unwrap();
                let kernel_vaddr = frame.ppn().start_addr().to_vaddr();

                unsafe {
                    core::ptr::copy_nonoverlapping(
                        (kernel_vaddr.as_usize() + page_offset) as *const u8,
                        buf[bytes_read..].as_mut_ptr(),
                        read_len,
                    );
                }
            }

            bytes_read += read_len;
        }

        self.update_atime();
        Ok(bytes_read)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        let meta = self.metadata.lock();

        if meta.inode_type != InodeType::File {
            return Err(FsError::IsDirectory);
        }

        drop(meta);

        let mut data = self.data.lock();
        let mut bytes_written = 0;

        while bytes_written < buf.len() {
            let page_index = (offset + bytes_written) / PAGE_SIZE;
            let page_offset = (offset + bytes_written) % PAGE_SIZE;
            let write_len = (PAGE_SIZE - page_offset).min(buf.len() - bytes_written);

            // 确保 Vec 足够大
            if page_index >= data.len() {
                data.resize(page_index + 1, None);
            }

            // 按需分配物理帧
            if data[page_index].is_none() {
                if self.reserve_page().is_err() {
                    return Err(FsError::NoSpace);
                }

                match alloc_frame() {
                    Some(frame) => {
                        data[page_index] = Some(Arc::new(frame));
                    }
                    None => {
                        // 如果物理帧分配失败，回滚预留的页面计数
                        self.cancel_page_reservation();
                        return Err(FsError::NoSpace);
                    }
                }
            }

            // 通过内核直接映射写入
            let frame = data[page_index].as_ref().unwrap();
            let kernel_vaddr = frame.ppn().start_addr().to_vaddr();

            unsafe {
                core::ptr::copy_nonoverlapping(
                    buf[bytes_written..].as_ptr(),
                    (kernel_vaddr.as_usize() + page_offset) as *mut u8,
                    write_len,
                );
            }

            bytes_written += write_len;
        }

        drop(data);

        // 更新文件大小和时间
        let mut meta = self.metadata.lock();
        meta.size = meta.size.max(offset + bytes_written);
        meta.blocks = (meta.size + 511) / 512; // 以 512B 为单位
        drop(meta);

        self.update_mtime();
        Ok(bytes_written)
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, FsError> {
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        drop(meta);

        let children = self.children.lock();

        // 处理特殊目录项
        if name == "." {
            // 返回自身
            let self_weak = self.self_ref.lock();
            if let Some(self_arc) = self_weak.upgrade() {
                return Ok(self_arc as Arc<dyn Inode>);
            }
            return Err(FsError::IoError); // 不应该发生
        } else if name == ".." {
            let parent = self.parent.lock();
            if let Some(parent_arc) = parent.upgrade() {
                return Ok(parent_arc as Arc<dyn Inode>);
            }
            // 根目录的 ".." 指向自己
            let self_weak = self.self_ref.lock();
            if let Some(self_arc) = self_weak.upgrade() {
                return Ok(self_arc as Arc<dyn Inode>);
            }
            return Err(FsError::IoError); // 不应该发生
        }

        children
            .get(name)
            .cloned()
            .map(|inode| inode as Arc<dyn Inode>)
            .ok_or(FsError::NotFound)
    }

    fn create(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        drop(meta);

        let mut children = self.children.lock();

        // 检查是否已存在
        if children.contains_key(name) {
            return Err(FsError::AlreadyExists);
        }

        // 创建新的 inode
        let inode_no = self.alloc_inode_no();

        // 获取自身的弱引用作为父节点
        let parent_weak = self.self_ref.lock().clone();

        let new_inode = TmpfsInode::new(
            inode_no,
            InodeType::File,
            mode,
            parent_weak,
            self.stats.clone(),
        );

        // 设置新文件的自引用
        *new_inode.self_ref.lock() = Arc::downgrade(&new_inode);

        children.insert(String::from(name), new_inode.clone());
        drop(children);

        self.update_mtime();

        Ok(new_inode as Arc<dyn Inode>)
    }

    fn mkdir(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        drop(meta);

        let mut children = self.children.lock();

        // 检查是否已存在
        if children.contains_key(name) {
            return Err(FsError::AlreadyExists);
        }

        // 创建新的目录 inode
        let inode_no = self.alloc_inode_no();

        // 获取自身的弱引用作为父节点
        let parent_weak = self.self_ref.lock().clone();

        let new_inode = TmpfsInode::new(
            inode_no,
            InodeType::Directory,
            mode,
            parent_weak,
            self.stats.clone(),
        );

        // 设置新目录的自引用
        *new_inode.self_ref.lock() = Arc::downgrade(&new_inode);

        children.insert(String::from(name), new_inode.clone());
        drop(children);

        // 更新父目录的 nlinks（子目录的 .. 指向父目录）
        let mut meta = self.metadata.lock();
        meta.nlinks += 1;
        drop(meta);

        self.update_mtime();

        Ok(new_inode as Arc<dyn Inode>)
    }

    fn unlink(&self, name: &str) -> Result<(), FsError> {
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        drop(meta);

        let mut children = self.children.lock();

        // 检查是否存在
        let child = children.get(name).ok_or(FsError::NotFound)?;

        // 检查是否是目录
        let child_meta = child.metadata.lock();
        if child_meta.inode_type == InodeType::Directory {
            return Err(FsError::IsDirectory);
        }
        drop(child_meta);

        // 获取该 inode 的已分配页数
        let child_data = child.data.lock();
        let allocated = child_data.iter().filter(|f| f.is_some()).count();
        drop(child_data);

        // 删除
        children.remove(name);
        self.dec_allocated_pages(allocated);
        self.update_mtime();

        Ok(())
    }

    fn rmdir(&self, name: &str) -> Result<(), FsError> {
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        drop(meta);

        let mut children = self.children.lock();

        // 检查是否存在
        let child = children.get(name).ok_or(FsError::NotFound)?;

        // 检查是否是目录
        let child_meta = child.metadata.lock();
        if child_meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        drop(child_meta);

        // 检查目录是否为空
        let child_children = child.children.lock();
        if !child_children.is_empty() {
            return Err(FsError::DirectoryNotEmpty);
        }
        drop(child_children);

        // 删除
        children.remove(name);

        // 更新父目录的 nlinks
        let mut meta = self.metadata.lock();
        meta.nlinks = meta.nlinks.saturating_sub(1);
        drop(meta);

        self.update_mtime();

        Ok(())
    }

    fn readdir(&self) -> Result<Vec<DirEntry>, FsError> {
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        let inode_no = meta.inode_no;
        drop(meta);

        let children = self.children.lock();
        let mut entries = Vec::new();

        // 添加 "." 和 ".."
        entries.push(DirEntry {
            name: String::from("."),
            inode_no,
            inode_type: InodeType::Directory,
        });

        let parent = self.parent.lock();
        let parent_inode_no = if let Some(parent_arc) = parent.upgrade() {
            parent_arc.metadata.lock().inode_no
        } else {
            inode_no // 根目录的 ".." 指向自己
        };
        drop(parent);

        entries.push(DirEntry {
            name: String::from(".."),
            inode_no: parent_inode_no,
            inode_type: InodeType::Directory,
        });

        // 添加子项
        for (name, child) in children.iter() {
            let child_meta = child.metadata.lock();
            entries.push(DirEntry {
                name: name.clone(),
                inode_no: child_meta.inode_no,
                inode_type: child_meta.inode_type,
            });
        }

        Ok(entries)
    }

    fn truncate(&self, new_size: usize) -> Result<(), FsError> {
        let mut meta = self.metadata.lock();

        if meta.inode_type != InodeType::File {
            return Err(FsError::IsDirectory);
        }

        let old_size = meta.size;

        if new_size < old_size {
            // 缩小：释放多余的页
            let new_page_count = (new_size + PAGE_SIZE - 1) / PAGE_SIZE;
            let old_page_count = (old_size + PAGE_SIZE - 1) / PAGE_SIZE;

            let mut data = self.data.lock();

            // 计算要释放的页数
            let pages_to_free = data[new_page_count..old_page_count.min(data.len())]
                .iter()
                .filter(|f| f.is_some())
                .count();

            data.truncate(new_page_count);
            drop(data);

            self.dec_allocated_pages(pages_to_free);
        }

        meta.size = new_size;
        meta.blocks = (new_size + 511) / 512;
        drop(meta);

        self.update_mtime();
        Ok(())
    }

    fn sync(&self) -> Result<(), FsError> {
        // tmpfs 完全在内存中，无需同步
        Ok(())
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn symlink(&self, name: &str, target: &str) -> Result<Arc<dyn Inode>, FsError> {
        // 检查当前节点是否为目录
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        drop(meta);

        // 检查文件名是否已存在
        let children = self.children.lock();
        if children.contains_key(name) {
            return Err(FsError::AlreadyExists);
        }
        drop(children);

        // 分配新的 inode 编号
        let mut stats = self.stats.lock();
        let inode_no = stats.next_inode_no;
        stats.next_inode_no += 1;
        drop(stats);

        // 创建符号链接 inode (默认权限 0o777)
        let symlink_inode = TmpfsInode::new(
            inode_no,
            InodeType::Symlink,
            FileMode::from_bits_truncate(0o777),
            Arc::downgrade(&self.self_ref.lock().upgrade().unwrap()),
            self.stats.clone(),
        );

        // 将目标路径写入符号链接文件的数据中
        let target_bytes = target.as_bytes();
        if let Err(e) = symlink_inode.write_at(0, target_bytes) {
            return Err(e);
        }

        // 添加到父目录
        self.children
            .lock()
            .insert(name.to_string(), symlink_inode.clone());

        // 更新父目录的修改时间
        self.metadata.lock().mtime = TimeSpec::now();

        Ok(symlink_inode as Arc<dyn Inode>)
    }

    fn link(&self, _name: &str, _target: &Arc<dyn Inode>) -> Result<(), FsError> {
        // TODO: 实现硬链接支持
        Err(FsError::NotSupported)
    }

    fn rename(
        &self,
        _old_name: &str,
        _new_parent: Arc<dyn Inode>,
        _new_name: &str,
    ) -> Result<(), FsError> {
        // TODO: 实现重命名支持
        Err(FsError::NotSupported)
    }

    fn set_times(&self, atime: Option<TimeSpec>, mtime: Option<TimeSpec>) -> Result<(), FsError> {
        let mut metadata = self.metadata.lock();
        if let Some(atime) = atime {
            metadata.atime = atime;
        }
        if let Some(mtime) = mtime {
            metadata.mtime = mtime;
        }
        Ok(())
    }

    fn readlink(&self) -> Result<String, FsError> {
        // 检查是否为符号链接
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Symlink {
            return Err(FsError::InvalidArgument);
        }
        let size = meta.size;
        drop(meta);

        // 读取符号链接的目标路径
        if size == 0 {
            return Ok(String::new());
        }

        let mut buf = alloc::vec![0u8; size];
        let bytes_read = self.read_at(0, &mut buf)?;

        // 转换为字符串
        String::from_utf8(buf[..bytes_read].to_vec()).map_err(|_| FsError::InvalidArgument)
    }

    fn mknod(&self, name: &str, mode: FileMode, dev: u64) -> Result<Arc<dyn Inode>, FsError> {
        // 检查当前节点是否为目录
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        drop(meta);

        // 检查文件名是否已存在
        let mut children = self.children.lock();
        if children.contains_key(name) {
            return Err(FsError::AlreadyExists);
        }

        // 从 mode 提取文件类型
        let inode_type = if mode.contains(FileMode::S_IFCHR) {
            InodeType::CharDevice
        } else if mode.contains(FileMode::S_IFBLK) {
            InodeType::BlockDevice
        } else if mode.contains(FileMode::S_IFIFO) {
            InodeType::Fifo
        } else {
            // mknod 只支持特殊文件
            return Err(FsError::InvalidArgument);
        };

        // 分配新的 inode 号
        let inode_no = self.alloc_inode_no();

        // 获取父节点的弱引用
        let parent_weak = self.self_ref.lock().clone();

        // 创建新的 inode
        let new_inode =
            TmpfsInode::new(inode_no, inode_type, mode, parent_weak, self.stats.clone());

        // 设置设备号 与 自引用
        new_inode.metadata.lock().rdev = dev;
        *new_inode.self_ref.lock() = Arc::downgrade(&new_inode);

        // 添加到父目录的子节点
        children.insert(String::from(name), new_inode.clone());
        drop(children);

        self.update_mtime();

        Ok(new_inode as Arc<dyn Inode>)
    }

    fn chmod(&self, _mode: FileMode) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn chown(&self, _uid: u32, _gid: u32) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }
}
