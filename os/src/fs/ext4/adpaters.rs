//! BlockDevice 适配器：BlockDriver → ext4_rs BlockDevice
//!
//! 负责在 Ext4 文件系统块大小 (4096 字节) 和 VirtIO 块设备扇区大小 (512 字节) 之间转换

use crate::config::VIRTIO_BLK_SECTOR_SIZE;
use crate::device::block::BlockDriver;
use alloc::sync::Arc;
use alloc::vec::Vec;

pub struct BlockDeviceAdapter {
    inner: Arc<dyn BlockDriver>,
    /// Ext4 文件系统块大小 (通常是 4096)
    block_size: usize,
    /// 底层设备扇区大小 (VirtIO 使用 512)
    sector_size: usize,
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
            block_size % sector_size == 0,
            "Block size must be a multiple of sector size"
        );

        Self {
            inner: device,
            block_size,
            sector_size,
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
        let sectors_needed =
            (sector_offset + bytes_needed + self.sector_size - 1) / self.sector_size;

        // 读取所有需要的扇区
        let mut buffer = alloc::vec![0u8; sectors_needed * self.sector_size];
        for i in 0..sectors_needed {
            let sector_id = start_sector + i;
            let buf_offset = i * self.sector_size;
            let sector_buf = &mut buffer[buf_offset..buf_offset + self.sector_size];

            if !self.inner.read_block(sector_id, sector_buf) {
                crate::pr_err!(
                    "[Ext4Adapter] Read error at sector {} (offset {})",
                    sector_id,
                    offset
                );
                return alloc::vec![0u8; self.block_size];
            }
        }

        // 从读取的数据中提取所需的 block_size 字节
        buffer[sector_offset..sector_offset + self.block_size].to_vec()
    }

    fn write_offset(&self, offset: usize, data: &[u8]) {
        // ext4_rs 期望将 data 写入从 offset 开始的位置
        // data 的长度通常等于 block_size

        // 计算起始扇区和扇区内偏移
        let start_sector = offset / self.sector_size;
        let sector_offset = offset % self.sector_size;

        // 计算需要写入多少个扇区
        let bytes_to_write = data.len();
        let sectors_needed =
            (sector_offset + bytes_to_write + self.sector_size - 1) / self.sector_size;

        // 读取-修改-写入
        let mut buffer = alloc::vec![0u8; sectors_needed * self.sector_size];

        // 1. 读取所有受影响的扇区
        for i in 0..sectors_needed {
            let sector_id = start_sector + i;
            let buf_offset = i * self.sector_size;
            let sector_buf = &mut buffer[buf_offset..buf_offset + self.sector_size];

            if !self.inner.read_block(sector_id, sector_buf) {
                crate::pr_err!(
                    "[Ext4Adapter] Read error at sector {} (for write, offset {})",
                    sector_id,
                    offset
                );
                return;
            }
        }

        // 2. 修改缓冲区
        buffer[sector_offset..sector_offset + bytes_to_write].copy_from_slice(data);

        // 3. 写回所有扇区
        for i in 0..sectors_needed {
            let sector_id = start_sector + i;
            let buf_offset = i * self.sector_size;
            let sector_data = &buffer[buf_offset..buf_offset + self.sector_size];

            if !self.inner.write_block(sector_id, sector_data) {
                crate::pr_err!(
                    "[Ext4Adapter] Write error at sector {} (offset {})",
                    sector_id,
                    offset
                );
            }
        }
    }
}
