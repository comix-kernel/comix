//! Ext4 Inode 包装
//!
//! 将 ext4_rs 的 inode 操作包装为 VFS Inode trait
//!
//! 设计要点：
//! - 使用 Dentry 引用而非存储路径，消除与 VFS 的冗余
//! - 需要路径时动态从 Dentry.full_path() 获取

use crate::sync::{Mutex, SpinLock};
use crate::uapi::time::TimeSpec;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use ext4_rs::InodeFileType;

use crate::vfs::dev::{
    decode_ext4_new_dev, decode_ext4_old_dev, encode_ext4_new_dev, encode_ext4_old_dev,
    major as dev_major, minor as dev_minor,
};
use crate::vfs::page_cache::{PAGE_CACHE_PAGE_SIZE, PageCache, PageCacheObjectId};
use crate::vfs::{Dentry, DirEntry, FileMode, FsError, Inode, InodeMetadata, InodeType};

const LOOKUP_CACHE_MAX_ENTRIES: usize = 4096;

struct LookupCache {
    entries: BTreeMap<u32, BTreeMap<String, u32>>,
    len: usize,
}

impl LookupCache {
    const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            len: 0,
        }
    }

    fn get(&self, parent_ino: u32, name: &str) -> Option<u32> {
        self.entries.get(&parent_ino)?.get(name).copied()
    }

    fn insert(&mut self, parent_ino: u32, name: &str, ino: u32) {
        let old = self
            .entries
            .entry(parent_ino)
            .or_default()
            .insert(String::from(name), ino);
        if old.is_none() {
            self.len += 1;
        }
        self.evict_if_needed();
    }

    fn remove(&mut self, parent_ino: u32, name: &str) {
        let Some(entries) = self.entries.get_mut(&parent_ino) else {
            return;
        };
        if entries.remove(name).is_some() {
            self.len = self.len.saturating_sub(1);
        }
        if entries.is_empty() {
            self.entries.remove(&parent_ino);
        }
    }

    fn evict_if_needed(&mut self) {
        while self.len > LOOKUP_CACHE_MAX_ENTRIES {
            let Some(parent_ino) = self.entries.keys().next().copied() else {
                self.len = 0;
                return;
            };
            let remove_parent = {
                let entries = self.entries.get_mut(&parent_ino).unwrap();
                if let Some(name) = entries.keys().next().cloned() {
                    entries.remove(&name);
                    self.len -= 1;
                }
                entries.is_empty()
            };
            if remove_parent {
                self.entries.remove(&parent_ino);
            }
        }
    }
}

pub struct Ext4InodeCaches {
    lookup: SpinLock<LookupCache>,
}

impl Ext4InodeCaches {
    pub const fn new() -> Self {
        Self {
            lookup: SpinLock::new(LookupCache::new()),
        }
    }
}

/// Ext4 Inode 包装
pub struct Ext4Inode {
    /// ext4_rs 文件系统对象
    fs: Arc<Mutex<ext4_rs::Ext4>>,

    /// Inode 号
    ino: u32,

    /// 关联的 Dentry（弱引用，避免循环引用）
    /// 用于获取完整路径，而不是在 Inode 中重复存储
    dentry: SpinLock<Weak<Dentry>>,

    /// Shared filesystem-level caches for regular reads and directory lookup.
    caches: Arc<Ext4InodeCaches>,

    /// Shared VFS clean page cache.
    page_cache: Arc<PageCache>,

    /// Stable filesystem instance id for page-cache object keys.
    fs_id: u64,
}

impl Ext4Inode {
    /// 创建新的 Ext4Inode
    ///
    /// 注意：初始创建时 dentry 为空，VFS 会在创建 Dentry 后调用 set_dentry()
    pub fn new(
        fs: Arc<Mutex<ext4_rs::Ext4>>,
        caches: Arc<Ext4InodeCaches>,
        page_cache: Arc<PageCache>,
        fs_id: u64,
        ino: u32,
    ) -> Self {
        Self {
            fs,
            ino,
            dentry: SpinLock::new(Weak::new()),
            caches,
            page_cache,
            fs_id,
        }
    }

    fn invalidate_read_cache(&self) {
        self.page_cache.invalidate_inode(self.cache_object_id());
    }

    fn invalidate_inode_no(&self, ino: u32) {
        self.page_cache
            .invalidate_inode(PageCacheObjectId::new(self.fs_id, ino as u64));
    }

    fn drop_lookup_cache_entry(&self, name: &str) {
        self.caches.lookup.lock().remove(self.ino, name);
    }

    fn cache_object_id(&self) -> PageCacheObjectId {
        PageCacheObjectId::new(self.fs_id, self.ino as u64)
    }

    #[cfg(test)]
    pub(crate) fn set_blocks_count_for_test(&self, blocks: u64) {
        let fs = self.fs.lock();
        let mut inode_ref = fs.get_inode_ref(self.ino);
        inode_ref.inode.set_blocks_count(blocks);
        fs.write_back_inode(&mut inode_ref);
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

    fn mode_file_type(mode: FileMode) -> Result<InodeFileType, FsError> {
        match mode & FileMode::S_IFMT {
            FileMode::S_IFREG => Ok(InodeFileType::S_IFREG),
            FileMode::S_IFCHR => Ok(InodeFileType::S_IFCHR),
            FileMode::S_IFBLK => Ok(InodeFileType::S_IFBLK),
            FileMode::S_IFIFO => Ok(InodeFileType::S_IFIFO),
            FileMode::S_IFSOCK => Ok(InodeFileType::S_IFSOCK),
            _ => Err(FsError::InvalidArgument),
        }
    }

    fn regular_file_mode(mode: FileMode) -> u16 {
        let permissions = mode.bits() & !FileMode::S_IFMT.bits();
        (InodeFileType::S_IFREG.bits() as u32 | permissions) as u16
    }

    fn directory_mode(mode: FileMode) -> u16 {
        let permissions = mode.bits() & !FileMode::S_IFMT.bits();
        (InodeFileType::S_IFDIR.bits() as u32 | permissions) as u16
    }

    fn special_file_mode(mode: FileMode, file_type: InodeFileType) -> u16 {
        let permissions = mode.bits() & !FileMode::S_IFMT.bits();
        (file_type.bits() as u32 | permissions) as u16
    }

    fn is_device_inode(inode_type: InodeType) -> bool {
        matches!(inode_type, InodeType::CharDevice | InodeType::BlockDevice)
    }

    fn read_rdev_from_blocks(block: &[u32; 15]) -> u64 {
        if block[0] != 0 {
            decode_ext4_old_dev(block[0])
        } else {
            decode_ext4_new_dev(block[1])
        }
    }

    fn encoded_rdev_blocks(dev: u64) -> [u32; 15] {
        let mut blocks = [0; 15];
        if dev_major(dev) <= 0xff && dev_minor(dev) <= 0xff {
            blocks[0] = encode_ext4_old_dev(dev);
        } else {
            blocks[1] = encode_ext4_new_dev(dev);
        }
        blocks
    }

    fn read_fast_symlink_target(&self, size: usize) -> Result<Option<String>, FsError> {
        const FAST_SYMLINK_MAX_LEN: usize = 15 * core::mem::size_of::<u32>();

        if size >= FAST_SYMLINK_MAX_LEN {
            return Ok(None);
        }

        let fs = self.fs.lock();
        let inode_ref = fs.get_inode_ref(self.ino);
        let inode = &inode_ref.inode;

        let mut buf = Vec::with_capacity(FAST_SYMLINK_MAX_LEN);
        for block_word in inode.block {
            buf.extend_from_slice(&block_word.to_le_bytes());
        }
        buf.truncate(size);

        String::from_utf8(buf)
            .map(Some)
            .map_err(|_| FsError::InvalidArgument)
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

        // 解析时间戳(包含纳秒精度)
        // extra 字段格式: (nanosec << 2) | epoch_bits
        // 我们只需要纳秒部分: extra >> 2
        let atime_nsec = (inode.i_atime_extra >> 2) as i64;
        let mtime_nsec = (inode.i_mtime_extra >> 2) as i64;
        let ctime_nsec = (inode.i_ctime_extra >> 2) as i64;

        Ok(InodeMetadata {
            inode_no: self.ino as usize,
            size: size as usize,
            blocks: inode.blocks as usize,
            atime: TimeSpec {
                tv_sec: inode.atime as i64,
                tv_nsec: atime_nsec,
            },
            mtime: TimeSpec {
                tv_sec: inode.mtime as i64,
                tv_nsec: mtime_nsec,
            },
            ctime: TimeSpec {
                tv_sec: inode.ctime as i64,
                tv_nsec: ctime_nsec,
            },
            inode_type,
            mode: FileMode::from_bits_truncate(mode as u32),
            nlinks: inode.links_count as usize,
            uid: inode.uid as u32,
            gid: inode.gid as u32,
            rdev: if Self::is_device_inode(inode_type) {
                Self::read_rdev_from_blocks(&inode.block)
            } else {
                0
            },
        })
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        if buf.is_empty() {
            return Ok(0);
        }

        // Check if this is a directory
        let metadata = self.metadata()?;
        if metadata.inode_type == InodeType::Directory {
            return Err(FsError::IsDirectory);
        }

        if offset >= metadata.size {
            return Ok(0);
        }

        let target_len = buf.len().min(metadata.size - offset);
        let object = self.cache_object_id();
        let mut copied = 0;
        while copied < target_len {
            let current_offset = offset + copied;
            let page_index = current_offset / PAGE_CACHE_PAGE_SIZE;
            let page_offset = current_offset % PAGE_CACHE_PAGE_SIZE;
            let chunk_len = (PAGE_CACHE_PAGE_SIZE - page_offset).min(target_len - copied);

            if let Some(n) = self.page_cache.read_hit(
                object,
                current_offset,
                &mut buf[copied..copied + chunk_len],
            ) {
                copied += n;
                if n == 0 {
                    break;
                }
                continue;
            }

            let page_start = page_index * PAGE_CACHE_PAGE_SIZE;
            let page_len = PAGE_CACHE_PAGE_SIZE.min(metadata.size - page_start);
            let mut page_buf = alloc::vec![0u8; page_len];
            let nread = {
                let fs = self.fs.lock();
                let nread = fs
                    .read_at(self.ino, page_start, &mut page_buf)
                    .map_err(|_| FsError::IoError)?;
                page_buf.truncate(nread);
                self.page_cache
                    .insert_clean(object, page_index, page_buf.clone());
                nread
            };

            if nread == 0 || page_offset >= page_buf.len() {
                break;
            }

            let n = (page_buf.len() - page_offset).min(chunk_len);
            buf[copied..copied + n].copy_from_slice(&page_buf[page_offset..page_offset + n]);
            copied += n;

            if n == 0 {
                break;
            }
        }

        Ok(copied)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        // Check if this is a directory
        let metadata = self.metadata()?;
        if metadata.inode_type == InodeType::Directory {
            return Err(FsError::IsDirectory);
        }

        let fs = self.fs.lock();

        // ext4_rs 的 write_at 签名: pub fn write_at(&self, inode: u32, offset: usize, write_buf: &[u8])
        let written = fs
            .write_at(self.ino, offset, buf)
            .map_err(|_| FsError::IoError)?;
        self.invalidate_read_cache();
        Ok(written)
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, FsError> {
        // Check if current inode is a directory
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        if let Some(child_ino) = self.caches.lookup.lock().get(self.ino, name) {
            return Ok(Arc::new(Ext4Inode::new(
                self.fs.clone(),
                self.caches.clone(),
                self.page_cache.clone(),
                self.fs_id,
                child_ino,
            )));
        }

        // 类似 create,lookup 也应该使用相对路径
        // 直接在当前目录下查找指定名称的文件
        let fs = self.fs.lock();
        let mut parent = self.ino;
        let mut name_off = 0;

        // 直接使用文件名作为路径
        let child_ino = fs
            .generic_open(name, &mut parent, false, 0, &mut name_off)
            .map_err(|_| FsError::NotFound)?;
        self.caches.lookup.lock().insert(self.ino, name, child_ino);

        // 创建子 Inode（暂时没有 dentry，VFS 会调用 set_dentry）
        Ok(Arc::new(Ext4Inode::new(
            self.fs.clone(),
            self.caches.clone(),
            self.page_cache.clone(),
            self.fs_id,
            child_ino,
        )))
    }

    fn create(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        // Check if current inode is a directory
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        // Check if file already exists
        if self.lookup(name).is_ok() {
            return Err(FsError::AlreadyExists);
        }

        let fs = self.fs.lock();
        let file_mode = Self::regular_file_mode(mode);

        let mut child_inode = fs
            .create(self.ino, name, file_mode)
            .map_err(|_| FsError::IoError)?;
        child_inode.inode.set_mode(file_mode);
        fs.write_back_inode(&mut child_inode);

        self.drop_lookup_cache_entry(name);
        self.invalidate_inode_no(child_inode.inode_num);
        Ok(Arc::new(Ext4Inode::new(
            self.fs.clone(),
            self.caches.clone(),
            self.page_cache.clone(),
            self.fs_id,
            child_inode.inode_num,
        )))
    }

    fn mkdir(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        // Check if current inode is a directory
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        // Check if directory already exists
        if self.lookup(name).is_ok() {
            return Err(FsError::AlreadyExists);
        }

        let fs = self.fs.lock();
        let dir_mode = Self::directory_mode(mode);

        let mut parent = self.ino;
        let mut name_off = 0;

        let inode_id = fs
            .generic_open(name, &mut parent, true, dir_mode, &mut name_off)
            .map_err(|e| {
                crate::pr_debug!("[Ext4Inode::mkdir] generic_open failed: {:?}", e);
                FsError::NoSpace
            })?;
        let mut inode_ref = fs.get_inode_ref(inode_id);
        inode_ref.inode.set_mode(dir_mode);
        fs.write_back_inode(&mut inode_ref);

        self.drop_lookup_cache_entry(name);
        self.invalidate_inode_no(inode_id);
        Ok(Arc::new(Ext4Inode::new(
            self.fs.clone(),
            self.caches.clone(),
            self.page_cache.clone(),
            self.fs_id,
            inode_id,
        )))
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

        const FAST_SYMLINK_MAX_LEN: usize = 15 * core::mem::size_of::<u32>();
        let target_bytes = target.as_bytes();
        if target_bytes.len() < FAST_SYMLINK_MAX_LEN {
            const EXT4_INODE_FLAG_EXTENTS: u32 = 0x0008_0000;

            let mut inode_ref = fs.get_inode_ref(new_inode.inode_num);
            inode_ref.inode.block = [0; 15];
            for (word, chunk) in inode_ref.inode.block.iter_mut().zip(target_bytes.chunks(4)) {
                let mut bytes = [0u8; 4];
                bytes[..chunk.len()].copy_from_slice(chunk);
                *word = u32::from_le_bytes(bytes);
            }
            inode_ref.inode.flags &= !EXT4_INODE_FLAG_EXTENTS;
            inode_ref.inode.set_size(target_bytes.len() as u64);
            fs.write_back_inode(&mut inode_ref);
        } else {
            fs.write_at(new_inode.inode_num, 0, target_bytes)
                .map_err(|_| FsError::IoError)?;
        }

        self.drop_lookup_cache_entry(name);
        self.invalidate_inode_no(new_inode.inode_num);
        Ok(Arc::new(Ext4Inode::new(
            self.fs.clone(),
            self.caches.clone(),
            self.page_cache.clone(),
            self.fs_id,
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
            return Err(FsError::CrossDeviceLink);
        }

        let fs = self.fs.lock();
        let mut self_ref = fs.get_inode_ref(self.ino);
        let mut target_ref = fs.get_inode_ref(ext4_inode.ino);
        fs.link(&mut self_ref, &mut target_ref, name)
            .map_err(|_| FsError::NoSpace)?;

        self.drop_lookup_cache_entry(name);
        self.invalidate_inode_no(ext4_inode.ino);
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

        self.drop_lookup_cache_entry(name);
        self.invalidate_inode_no(child_ext4.ino);
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
            .map(|_| {
                self.drop_lookup_cache_entry(name);
                self.page_cache.invalidate_fs(self.fs_id);
            })
            .map_err(|_| FsError::NotFound)
    }

    /// 重命名或移动文件/目录
    ///
    /// # 安全性保证
    /// - **并发安全**：持有文件系统锁直到操作完成，防止竞态条件
    /// - **失败回滚**：关键操作失败时会尝试恢复到原始状态
    /// - **参数验证**：严格检查所有前置条件
    ///
    /// # 安全性限制 ⚠️
    /// - **非崩溃安全**：由于 ext4_rs 没有事务日志支持，系统崩溃时可能导致文件系统不一致
    ///   - 最坏情况：文件可能同时出现在两个位置，或完全丢失
    ///   - 建议：关键操作后调用 `sync()` 确保数据写入磁盘
    /// - **回滚非原子**：回滚操作本身也可能失败（如磁盘已满）
    /// - **简化的循环检测**：只检查是否移动到自身，未实现完整的祖先链遍历
    ///
    /// # 注意事项
    /// - 操作持有全局文件系统锁，可能影响并发性能
    /// - 跨目录移动目录比简单重命名更耗时（需要更新 ".." 引用）
    fn rename(
        &self,
        old_name: &str,
        new_parent: Arc<dyn Inode>,
        new_name: &str,
    ) -> Result<(), FsError> {
        // 重命名实现：使用 "添加新条目 + 删除旧条目" 的方式
        //
        // 操作顺序（最小化中间不一致状态）：
        // 1. 验证所有参数和前置条件
        // 2. 备份需要删除的目标（如果存在）
        // 3. 在新位置添加条目
        // 4. 从旧位置删除条目
        // 5. 如果是目录，更新 ".." 引用
        // 6. 失败时回滚

        // ========== 阶段 1: 验证 ==========

        // 检查当前 inode（旧父目录）是否为目录
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        // 查找要重命名的子项
        let old_child = self.lookup(old_name)?;
        let old_child_metadata = old_child.metadata()?;
        let old_child_ext4 = old_child
            .as_any()
            .downcast_ref::<Ext4Inode>()
            .ok_or(FsError::InvalidArgument)?;

        // 转换新父目录
        let new_parent_ext4 = new_parent
            .as_any()
            .downcast_ref::<Ext4Inode>()
            .ok_or(FsError::InvalidArgument)?;

        // 检查新父目录是否为目录
        let new_parent_metadata = new_parent_ext4.metadata()?;
        if new_parent_metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        // 确保在同一个文件系统中
        if !Arc::ptr_eq(&self.fs, &new_parent_ext4.fs) {
            return Err(FsError::CrossDeviceLink);
        }

        // 防止将目录移动到其子目录中（会造成循环）
        if old_child_metadata.inode_type == InodeType::Directory {
            // 简单检查：如果新父目录就是被移动的目录本身
            if old_child_ext4.ino == new_parent_ext4.ino {
                return Err(FsError::InvalidArgument);
            }
            // TODO: 完整的循环检查需要遍历新父目录的祖先链
        }

        // 持有锁直到操作完成
        let fs = self.fs.lock();

        // ========== 阶段 2: 检查目标是否存在 ==========

        let mut replaced_inode: Option<u32> = None;

        // 检查目标位置是否已有文件/目录
        let target_exists = {
            let mut parent = new_parent_ext4.ino;
            let mut name_off = 0;
            fs.generic_open(new_name, &mut parent, false, 0, &mut name_off)
                .ok()
        };

        if let Some(existing_ino) = target_exists {
            // 如果目标已存在，需要先删除它
            let existing_ref = fs.get_inode_ref(existing_ino);
            let replaced_is_dir = existing_ref.inode.is_dir();
            let old_child_is_dir = old_child_metadata.inode_type == InodeType::Directory;

            if old_child_is_dir && !replaced_is_dir {
                return Err(FsError::NotDirectory);
            }
            if !old_child_is_dir && replaced_is_dir {
                return Err(FsError::IsDirectory);
            }

            if replaced_is_dir {
                // 如果目标是目录，必须为空
                if fs.dir_has_entry(existing_ino) {
                    return Err(FsError::DirectoryNotEmpty);
                }

                // 删除空目录
                fs.dir_remove(new_parent_ext4.ino, new_name)
                    .map_err(|_| FsError::IoError)?;
            } else {
                // 删除普通文件
                let mut new_parent_ref = fs.get_inode_ref(new_parent_ext4.ino);
                let mut existing_ref = fs.get_inode_ref(existing_ino);

                fs.unlink(&mut new_parent_ref, &mut existing_ref, new_name)
                    .map_err(|_| FsError::IoError)?;

                fs.write_back_inode(&mut new_parent_ref);
            }

            // 记录被替换的 inode，以便回滚
            replaced_inode = Some(existing_ino);
        }

        // ========== 阶段 3: 执行重命名（关键部分）==========

        let mut old_parent_ref = fs.get_inode_ref(self.ino);
        let mut new_parent_ref = fs.get_inode_ref(new_parent_ext4.ino);
        let child_ref = fs.get_inode_ref(old_child_ext4.ino);

        // 步骤 3a: 在新位置添加条目
        if let Err(_e) = fs.dir_add_entry(&mut new_parent_ref, &child_ref, new_name) {
            // 失败：尝试恢复被删除的目标文件
            if let Some(replaced_ino) = replaced_inode {
                let replaced_ref = fs.get_inode_ref(replaced_ino);
                let _ = fs.dir_add_entry(&mut new_parent_ref, &replaced_ref, new_name);
                fs.write_back_inode(&mut new_parent_ref);
            }
            return Err(FsError::NoSpace);
        }

        // 步骤 3b: 从旧位置删除条目
        if let Err(_e) = fs.dir_remove_entry(&mut old_parent_ref, old_name) {
            // 回滚：删除刚添加的新条目
            let _ = fs.dir_remove_entry(&mut new_parent_ref, new_name);

            // 尝试恢复被删除的目标文件
            if let Some(replaced_ino) = replaced_inode {
                let replaced_ref = fs.get_inode_ref(replaced_ino);
                let _ = fs.dir_add_entry(&mut new_parent_ref, &replaced_ref, new_name);
            }

            fs.write_back_inode(&mut old_parent_ref);
            fs.write_back_inode(&mut new_parent_ref);
            return Err(FsError::IoError);
        }

        // ========== 阶段 4: 更新目录的 ".." 引用（如果需要）==========

        if old_child_metadata.inode_type == InodeType::Directory && self.ino != new_parent_ext4.ino
        {
            // 只有跨目录移动时才需要更新 ".."
            let mut child_ref = fs.get_inode_ref(old_child_ext4.ino);

            // 步骤 4a: 删除旧的 ".." 条目
            if let Err(_e) = fs.dir_remove_entry(&mut child_ref, "..") {
                // 回滚：恢复旧位置的条目，删除新位置的条目
                let _ = fs.dir_add_entry(&mut old_parent_ref, &child_ref, old_name);
                let _ = fs.dir_remove_entry(&mut new_parent_ref, new_name);

                if let Some(replaced_ino) = replaced_inode {
                    let replaced_ref = fs.get_inode_ref(replaced_ino);
                    let _ = fs.dir_add_entry(&mut new_parent_ref, &replaced_ref, new_name);
                }

                fs.write_back_inode(&mut old_parent_ref);
                fs.write_back_inode(&mut new_parent_ref);
                fs.write_back_inode(&mut child_ref);
                return Err(FsError::IoError);
            }

            // 步骤 4b: 添加新的 ".." 条目
            if let Err(_e) = fs.dir_add_entry(&mut child_ref, &new_parent_ref, "..") {
                // 回滚：恢复 ".." 条目，恢复旧位置条目，删除新位置条目
                let _ = fs.dir_add_entry(&mut child_ref, &old_parent_ref, "..");
                let _ = fs.dir_add_entry(&mut old_parent_ref, &child_ref, old_name);
                let _ = fs.dir_remove_entry(&mut new_parent_ref, new_name);

                if let Some(replaced_ino) = replaced_inode {
                    let replaced_ref = fs.get_inode_ref(replaced_ino);
                    let _ = fs.dir_add_entry(&mut new_parent_ref, &replaced_ref, new_name);
                }

                fs.write_back_inode(&mut old_parent_ref);
                fs.write_back_inode(&mut new_parent_ref);
                fs.write_back_inode(&mut child_ref);
                return Err(FsError::NoSpace);
            }

            // 步骤 4c: 更新链接计数
            let old_parent_links = old_parent_ref.inode.links_count();
            if old_parent_links > 0 {
                old_parent_ref.inode.set_links_count(old_parent_links - 1);
            }

            let new_parent_links = new_parent_ref.inode.links_count();
            new_parent_ref.inode.set_links_count(new_parent_links + 1);

            fs.write_back_inode(&mut child_ref);
        }

        // ========== 阶段 5: 提交更改 ==========

        fs.write_back_inode(&mut old_parent_ref);
        fs.write_back_inode(&mut new_parent_ref);

        self.drop_lookup_cache_entry(old_name);
        new_parent_ext4.drop_lookup_cache_entry(new_name);
        self.invalidate_inode_no(old_child_ext4.ino);
        if let Some(replaced_ino) = replaced_inode {
            self.invalidate_inode_no(replaced_ino);
        }
        Ok(())
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

        self.invalidate_read_cache();
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

    fn set_times(&self, atime: Option<TimeSpec>, mtime: Option<TimeSpec>) -> Result<(), FsError> {
        let fs = self.fs.lock();

        // 获取 inode 引用（可变）
        let mut inode_ref = fs.get_inode_ref(self.ino);
        let inode = &mut inode_ref.inode;

        // 更新访问时间
        if let Some(at) = atime {
            inode.atime = at.tv_sec as u32;
            // 设置纳秒精度: (nanosec << 2) | epoch_bits
            // epoch_bits 通常为 0，所以直接左移 2 位
            inode.i_atime_extra = ((at.tv_nsec as u32) << 2) & 0xFFFFFFFC;
        }

        // 更新修改时间
        if let Some(mt) = mtime {
            inode.mtime = mt.tv_sec as u32;
            inode.i_mtime_extra = ((mt.tv_nsec as u32) << 2) & 0xFFFFFFFC;

            // 修改时间改变时，也更新 ctime
            let now = TimeSpec::now();
            inode.ctime = now.tv_sec as u32;
            inode.i_ctime_extra = ((now.tv_nsec as u32) << 2) & 0xFFFFFFFC;
        }

        // 写回 inode 到磁盘
        fs.write_back_inode(&mut inode_ref);

        Ok(())
    }

    fn chown(&self, uid: u32, gid: u32) -> Result<(), FsError> {
        let fs = self.fs.lock();

        // 获取 inode 引用（可变）
        let mut inode_ref = fs.get_inode_ref(self.ino);
        let inode = &mut inode_ref.inode;

        // 更新 uid/gid（u32::MAX 表示不改变）
        if uid != u32::MAX {
            inode.uid = uid as u16;
            // 如果将来需要支持 32 位 UID，需要更新 i_uid_high
        }
        if gid != u32::MAX {
            inode.gid = gid as u16;
            // 如果将来需要支持 32 位 GID，需要更新 i_gid_high
        }

        // 更新 ctime（状态改变时间）
        let now = TimeSpec::now();
        inode.ctime = now.tv_sec as u32;
        inode.i_ctime_extra = ((now.tv_nsec as u32) << 2) & 0xFFFFFFFC;

        // 写回 inode 到磁盘
        fs.write_back_inode(&mut inode_ref);

        Ok(())
    }

    fn chmod(&self, mode: FileMode) -> Result<(), FsError> {
        let fs = self.fs.lock();

        // 获取 inode 引用（可变）
        let mut inode_ref = fs.get_inode_ref(self.ino);
        let inode = &mut inode_ref.inode;

        // 保留文件类型位（高 4 位），只修改权限位（低 12 位）
        let file_type = inode.mode & 0xF000;
        let permission_bits = (mode.bits() & 0x0FFF) as u16;
        inode.mode = file_type | permission_bits;

        // 更新 ctime（状态改变时间）
        let now = TimeSpec::now();
        inode.ctime = now.tv_sec as u32;
        inode.i_ctime_extra = ((now.tv_nsec as u32) << 2) & 0xFFFFFFFC;

        // 写回 inode 到磁盘
        fs.write_back_inode(&mut inode_ref);

        Ok(())
    }

    fn readlink(&self) -> Result<String, FsError> {
        // 检查是否为符号链接
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Symlink {
            return Err(FsError::InvalidArgument);
        }

        // 获取符号链接的大小（即目标路径的长度）
        let size = metadata.size;
        if size == 0 {
            return Ok(String::new());
        }

        if let Some(target) = self.read_fast_symlink_target(size)? {
            return Ok(target);
        }

        // 读取长符号链接目标。短符号链接已在上面从 inode 的 i_block 内联区读取。
        let fs = self.fs.lock();
        let mut buf = alloc::vec![0u8; size];

        let bytes_read = fs
            .read_at(self.ino, 0, &mut buf)
            .map_err(|_| FsError::IoError)?;

        // 截断到实际读取的长度
        buf.truncate(bytes_read);

        // 转换为UTF-8字符串
        String::from_utf8(buf).map_err(|_| FsError::InvalidArgument)
    }

    fn mknod(&self, name: &str, mode: FileMode, dev: u64) -> Result<Arc<dyn Inode>, FsError> {
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        if self.lookup(name).is_ok() {
            return Err(FsError::AlreadyExists);
        }

        let file_type = Self::mode_file_type(mode)?;
        let inode_mode = Self::special_file_mode(mode, file_type);
        let fs = self.fs.lock();

        let mut new_inode = fs
            .create(self.ino, name, inode_mode)
            .map_err(|_| FsError::IoError)?;

        new_inode.inode.set_mode(inode_mode);
        new_inode.inode.set_size(0);
        new_inode.inode.set_blocks_count(0);

        if matches!(file_type, InodeFileType::S_IFCHR | InodeFileType::S_IFBLK) {
            new_inode.inode.block = Self::encoded_rdev_blocks(dev);
        }

        fs.write_back_inode(&mut new_inode);

        self.drop_lookup_cache_entry(name);
        self.invalidate_inode_no(new_inode.inode_num);
        Ok(Arc::new(Ext4Inode::new(
            self.fs.clone(),
            self.caches.clone(),
            self.page_cache.clone(),
            self.fs_id,
            new_inode.inode_num,
        )))
    }
}
