//! 块设备文件的 File trait 实现

use crate::device::BLK_DRIVERS;
use crate::sync::SpinLock;
use crate::vfs::devno::get_blkdev_index;
use crate::vfs::{Dentry, File, FsError, Inode, InodeMetadata, OpenFlags, SeekWhence};
use alloc::sync::Arc;

/// 块设备文件
pub struct BlockDeviceFile {
    /// 关联的 dentry
    pub dentry: Arc<Dentry>,

    /// 关联的 inode
    pub inode: Arc<dyn Inode>,

    /// 设备号
    dev: u64,

    /// 块设备驱动索引（在 BLK_DRIVERS 中）
    blk_index: Option<usize>,

    /// 打开标志位
    pub flags: OpenFlags,

    /// 当前偏移量（字节）
    offset: SpinLock<usize>,
}

impl BlockDeviceFile {
    /// 创建新的块设备文件
    pub fn new(dentry: Arc<Dentry>, flags: OpenFlags) -> Result<Self, FsError> {
        let inode = dentry.inode.clone();
        let metadata = inode.metadata()?;
        let dev = metadata.rdev;

        // 查找块设备驱动
        let blk_index = get_blkdev_index(dev);

        if blk_index.is_none() {
            return Err(FsError::NoDevice);
        }

        Ok(Self {
            dentry,
            inode,
            dev,
            blk_index,
            flags,
            offset: SpinLock::new(0),
        })
    }

    /// 获取块大小（通常为 512 字节）
    const BLOCK_SIZE: usize = 512;
}

impl File for BlockDeviceFile {
    fn readable(&self) -> bool {
        self.flags.readable()
    }

    fn writable(&self) -> bool {
        self.flags.writable()
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        if !self.readable() {
            return Err(FsError::PermissionDenied);
        }

        let blk_idx = self.blk_index.ok_or(FsError::NoDevice)?;
        let drivers = BLK_DRIVERS.read();
        let driver = drivers.get(blk_idx).ok_or(FsError::NoDevice)?;

        let mut offset_guard = self.offset.lock();
        let current_offset = *offset_guard;

        // 计算起始扇区和扇区内偏移
        let start_sector = current_offset / Self::BLOCK_SIZE;
        let sector_offset = current_offset % Self::BLOCK_SIZE;

        let mut total_read = 0;
        let mut remaining = buf.len();

        // 读取数据（可能跨多个扇区）
        while remaining > 0 {
            let sector_idx = start_sector + total_read / Self::BLOCK_SIZE;
            let offset_in_sector = if total_read == 0 { sector_offset } else { 0 };
            let to_read = remaining.min(Self::BLOCK_SIZE - offset_in_sector);

            // 读取一个扇区
            let mut sector_buf = [0u8; 512];
            if !driver.read_block(sector_idx, &mut sector_buf) {
                return Err(FsError::IoError);
            }

            // 复制数据
            buf[total_read..total_read + to_read]
                .copy_from_slice(&sector_buf[offset_in_sector..offset_in_sector + to_read]);

            total_read += to_read;
            remaining -= to_read;
        }

        *offset_guard = current_offset + total_read;
        Ok(total_read)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        if !self.writable() {
            return Err(FsError::PermissionDenied);
        }

        let blk_idx = self.blk_index.ok_or(FsError::NoDevice)?;
        let drivers = BLK_DRIVERS.read();
        let driver = drivers.get(blk_idx).ok_or(FsError::NoDevice)?;

        let mut offset_guard = self.offset.lock();
        let current_offset = *offset_guard;

        let start_sector = current_offset / Self::BLOCK_SIZE;
        let sector_offset = current_offset % Self::BLOCK_SIZE;

        let mut total_written = 0;
        let mut remaining = buf.len();

        while remaining > 0 {
            let sector_idx = start_sector + total_written / Self::BLOCK_SIZE;
            let offset_in_sector = if total_written == 0 { sector_offset } else { 0 };
            let to_write = remaining.min(Self::BLOCK_SIZE - offset_in_sector);

            let mut sector_buf = [0u8; 512];

            // 如果不是完整扇区写入，需要先读取
            if offset_in_sector != 0 || to_write != Self::BLOCK_SIZE {
                if !driver.read_block(sector_idx, &mut sector_buf) {
                    return Err(FsError::IoError);
                }
            }

            // 修改数据
            sector_buf[offset_in_sector..offset_in_sector + to_write]
                .copy_from_slice(&buf[total_written..total_written + to_write]);

            // 写回
            if !driver.write_block(sector_idx, &sector_buf) {
                return Err(FsError::IoError);
            }

            total_written += to_write;
            remaining -= to_write;
        }

        *offset_guard = current_offset + total_written;
        Ok(total_written)
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        self.inode.metadata()
    }

    fn lseek(&self, offset: isize, whence: SeekWhence) -> Result<usize, FsError> {
        let blk_idx = self.blk_index.ok_or(FsError::NoDevice)?;
        let drivers = BLK_DRIVERS.read();
        let driver = drivers.get(blk_idx).ok_or(FsError::NoDevice)?;

        let device_size = driver.total_blocks() * Self::BLOCK_SIZE;

        let mut offset_guard = self.offset.lock();
        let current = *offset_guard as isize;

        let new_offset = match whence {
            SeekWhence::Set => offset,
            SeekWhence::Cur => current + offset,
            SeekWhence::End => device_size as isize + offset,
        };

        if new_offset < 0 {
            return Err(FsError::InvalidArgument);
        }

        *offset_guard = new_offset as usize;
        Ok(new_offset as usize)
    }

    fn offset(&self) -> usize {
        *self.offset.lock()
    }

    fn flags(&self) -> OpenFlags {
        self.flags.clone()
    }

    fn inode(&self) -> Result<Arc<dyn Inode>, FsError> {
        Ok(self.inode.clone())
    }

    fn dentry(&self) -> Result<Arc<Dentry>, FsError> {
        Ok(self.dentry.clone())
    }
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
