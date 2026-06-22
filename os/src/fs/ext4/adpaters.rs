//! BlockDevice 适配器：BlockDriver → ext4_rs BlockDevice
//!
//! 负责在 Ext4 文件系统块大小 (4096 字节) 和 VirtIO 块设备扇区大小 (512 字节) 之间转换

use crate::config::VIRTIO_BLK_SECTOR_SIZE;
use crate::device::block::BlockDriver;
use crate::sync::SpinLock;
use alloc::vec::Vec;
use alloc::{collections::BTreeMap, sync::Arc};

const READ_CACHE_BLOCKS: usize = 1024;

struct CacheEntry {
    data: Vec<u8>,
    last_used: usize,
}

struct BlockReadCache {
    entries: BTreeMap<usize, CacheEntry>,
    clock: usize,
}

impl BlockReadCache {
    fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            clock: 0,
        }
    }

    fn next_clock(&mut self) -> usize {
        self.clock = self.clock.wrapping_add(1);
        self.clock
    }

    fn get(&mut self, offset: usize) -> Option<Vec<u8>> {
        let last_used = self.next_clock();
        let entry = self.entries.get_mut(&offset)?;
        entry.last_used = last_used;
        Some(entry.data.clone())
    }

    fn insert(&mut self, offset: usize, data: Vec<u8>) {
        let last_used = self.next_clock();
        if self.entries.len() >= READ_CACHE_BLOCKS
            && !self.entries.contains_key(&offset)
            && let Some((&evict_offset, _)) = self.entries.iter().min_by_key(|(_, e)| e.last_used)
        {
            self.entries.remove(&evict_offset);
        }

        self.entries.insert(offset, CacheEntry { data, last_used });
    }

    fn invalidate_range(&mut self, start: usize, end: usize, block_size: usize) {
        if start >= end {
            return;
        }

        let stale_offsets: Vec<usize> = self
            .entries
            .keys()
            .copied()
            .filter(|&offset| {
                let block_end = offset.saturating_add(block_size);
                offset < end && start < block_end
            })
            .collect();

        for offset in stale_offsets {
            self.entries.remove(&offset);
        }
    }
}

pub struct BlockDeviceAdapter {
    inner: Arc<dyn BlockDriver>,
    /// Ext4 文件系统块大小 (通常是 4096)
    block_size: usize,
    /// 底层设备扇区大小 (VirtIO 使用 512)
    sector_size: usize,
    /// 小型读缓存，避免动态加载器和目录遍历重复触发慢速 VirtIO 读。
    read_cache: SpinLock<BlockReadCache>,
}

impl BlockDeviceAdapter {
    pub fn new(device: Arc<dyn BlockDriver>, block_size: usize) -> Self {
        let sector_size = VIRTIO_BLK_SECTOR_SIZE;
        crate::pr_info!(
            "[Ext4Adapter] Created adapter: ext4_block_size={}, sector_size={}",
            block_size,
            sector_size
        );

        // 确保块大小是扇区大小的整数倍
        assert!(
            block_size.is_multiple_of(sector_size),
            "Block size must be a multiple of sector size"
        );

        Self {
            inner: device,
            block_size,
            sector_size,
            read_cache: SpinLock::new(BlockReadCache::new()),
        }
    }
}

impl ext4_rs::BlockDevice for BlockDeviceAdapter {
    fn read_offset(&self, offset: usize) -> Vec<u8> {
        // ext4_rs 期望读取从 offset 开始的 block_size 字节数据
        // 我们需要将这个请求转换为多个扇区读取

        // 计算起始扇区和扇区内偏移
        let start_sector = offset / self.sector_size;
        let sector_offset = offset % self.sector_size;

        // 计算需要读取多少个扇区才能获得 block_size 字节的数据
        let bytes_needed = self.block_size;
        let sectors_needed = (sector_offset + bytes_needed).div_ceil(self.sector_size);

        if sector_offset == 0
            && let Some(cached) = self.read_cache.lock().get(offset)
        {
            return cached;
        }

        // 读取所有需要的扇区。VirtIO 驱动支持一次读取连续扇区，
        // 对 4K ext4 块可把 8 次 512B 请求合并成 1 次。
        let mut buffer = alloc::vec![0u8; sectors_needed * self.sector_size];
        if !self.inner.read_blocks(start_sector, &mut buffer) {
            crate::pr_err!(
                "[Ext4Adapter] Read error at sector {} count {} (offset {})",
                start_sector,
                sectors_needed,
                offset
            );
            return alloc::vec![0u8; self.block_size];
        }

        // 从读取的数据中提取所需的 block_size 字节
        if sector_offset == 0 {
            self.read_cache.lock().insert(offset, buffer.clone());
            buffer
        } else {
            buffer[sector_offset..sector_offset + self.block_size].to_vec()
        }
    }

    fn write_offset(&self, offset: usize, data: &[u8]) {
        // ext4_rs 期望将 data 写入从 offset 开始的位置
        // data 的长度通常等于 block_size

        // 计算起始扇区和扇区内偏移
        let start_sector = offset / self.sector_size;
        let sector_offset = offset % self.sector_size;

        // 计算需要写入多少个扇区
        let bytes_to_write = data.len();
        if bytes_to_write == 0 {
            return;
        }
        let sectors_needed = (sector_offset + bytes_to_write).div_ceil(self.sector_size);

        let Some(write_end) = offset.checked_add(bytes_to_write) else {
            crate::pr_err!("[Ext4Adapter] Write offset overflow at offset {}", offset);
            return;
        };

        self.read_cache
            .lock()
            .invalidate_range(offset, write_end, self.block_size);

        if sector_offset == 0 && bytes_to_write.is_multiple_of(self.sector_size) {
            if !self.inner.write_blocks(start_sector, data) {
                crate::pr_err!(
                    "[Ext4Adapter] Write error at sector {} count {} (offset {})",
                    start_sector,
                    sectors_needed,
                    offset
                );
            }
            return;
        }

        for i in 0..sectors_needed {
            let sector_id = start_sector + i;
            let sector_start = sector_id * self.sector_size;
            let sector_end = sector_start + self.sector_size;
            let overlap_start = offset.max(sector_start);
            let overlap_end = write_end.min(sector_end);
            let data_start = overlap_start - offset;
            let data_end = overlap_end - offset;

            if overlap_start == sector_start && overlap_end == sector_end {
                if !self
                    .inner
                    .write_blocks(sector_id, &data[data_start..data_end])
                {
                    crate::pr_err!(
                        "[Ext4Adapter] Write error at sector {} (offset {})",
                        sector_id,
                        offset
                    );
                }
                continue;
            }

            let mut sector_buf = alloc::vec![0u8; self.sector_size];
            if !self.inner.read_block(sector_id, &mut sector_buf) {
                crate::pr_err!(
                    "[Ext4Adapter] Read error at sector {} (for write, offset {})",
                    sector_id,
                    offset
                );
                return;
            }
            let sector_buf_start = overlap_start - sector_start;
            let sector_buf_end = sector_buf_start + (overlap_end - overlap_start);
            sector_buf[sector_buf_start..sector_buf_end]
                .copy_from_slice(&data[data_start..data_end]);

            if !self.inner.write_block(sector_id, &sector_buf) {
                crate::pr_err!(
                    "[Ext4Adapter] Write error at sector {} (offset {})",
                    sector_id,
                    offset
                );
            }
        }
    }
}
