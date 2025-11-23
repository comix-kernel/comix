//! BlockDevice 适配器：BlockDriver → ext4_rs BlockDevice
//! 纯净版：移除了所有针对 ENOSPC 的 Hack

use crate::device::block::BlockDriver;
use alloc::sync::Arc;
use alloc::vec::Vec;

pub struct BlockDeviceAdapter {
    inner: Arc<dyn BlockDriver>,
    block_size: usize,
}

impl BlockDeviceAdapter {
    pub fn new(device: Arc<dyn BlockDriver>, block_size: usize) -> Self {
        crate::println!(
            "[Ext4Adapter] Created adapter with block_size: {}",
            block_size
        );
        Self {
            inner: device,
            block_size,
        }
    }
}

impl ext4_rs::BlockDevice for BlockDeviceAdapter {
    fn read_offset(&self, offset: usize) -> Vec<u8> {
        let block_id = offset / self.block_size;
        let block_offset = offset % self.block_size;

        // 标准读取逻辑
        let mut buf = alloc::vec![0u8; self.block_size];
        if self.inner.read_block(block_id, &mut buf) {
            // 移除了所有 if offset == 1024 (Superblock Spoofing) 的代码
            // 移除了所有 if block_id == 1 (GDT Spoofing) 的代码
            // 移除了所有 mask RO_COMPAT 的代码 (如果在 mkfs 时处理得当)

            // 处理跨块读取的逻辑保持不变
            if block_offset == 0 {
                buf
            } else {
                let mut result = buf[block_offset..].to_vec();
                let mut next_buf = alloc::vec![0u8; self.block_size];
                if self.inner.read_block(block_id + 1, &mut next_buf) {
                    result.extend_from_slice(&next_buf[..block_offset]);
                }
                result
            }
        } else {
            crate::pr_err!("[Ext4Adapter] Read error at offset {}", offset);
            alloc::vec![0u8; self.block_size]
        }
    }

    fn write_offset(&self, offset: usize, data: &[u8]) {
        // 写入逻辑本来就是正常的，不需要改动
        let block_id = offset / self.block_size;
        let block_offset = offset % self.block_size;

        if block_offset == 0 && data.len() == self.block_size {
            if !self.inner.write_block(block_id, data) {
                crate::pr_err!("[Ext4Adapter] Write error at offset {}", offset);
            }
            return;
        }

        let mut buf = alloc::vec![0u8; self.block_size];
        if self.inner.read_block(block_id, &mut buf) {
            let space_in_block = self.block_size - block_offset;
            let write_len = data.len().min(space_in_block);
            buf[block_offset..block_offset + write_len].copy_from_slice(&data[..write_len]);

            if !self.inner.write_block(block_id, &buf) {
                crate::pr_err!("[Ext4Adapter] Write error at offset {}", offset);
            }

            if write_len < data.len() {
                let remaining_data = &data[write_len..];
                let next_block_id = block_id + 1;
                let mut next_buf = alloc::vec![0u8; self.block_size];
                if self.inner.read_block(next_block_id, &mut next_buf) {
                    let next_write_len = remaining_data.len().min(self.block_size);
                    next_buf[..next_write_len].copy_from_slice(&remaining_data[..next_write_len]);
                    if !self.inner.write_block(next_block_id, &next_buf) {
                        crate::pr_err!("[Ext4Adapter] Write error at offset {}", offset);
                    }
                }
            }
        }
    }
}
