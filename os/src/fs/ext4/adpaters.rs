//! BlockDevice 适配器：VFS BlockDevice → ext4_rs BlockDevice

use crate::pr_err;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::devices::{BlockDevice as VfsBlockDevice, block_device::BlockError};

/// ext4_rs BlockDevice trait 实现
///
/// 将我们的 VFS BlockDevice 适配为 ext4_rs 要求的接口
pub struct BlockDeviceAdapter {
    /// 底层块设备 (RamDisk 或 VirtIOBlock)
    inner: Arc<dyn VfsBlockDevice>,
    /// 块大小（通常 4096 字节）
    block_size: usize,
}

impl BlockDeviceAdapter {
    pub fn new(device: Arc<dyn VfsBlockDevice>) -> Self {
        let block_size = device.block_size();
        Self {
            inner: device,
            block_size,
        }
    }
}

/// 实现 ext4_rs 的 BlockDevice trait
impl ext4_rs::BlockDevice for BlockDeviceAdapter {
    fn read_offset(&self, offset: usize) -> Vec<u8> {
        // ext4_rs 可能读取非对齐的数据（如超级块在偏移1024处）
        // 需要处理跨块读取
        let block_id = offset / self.block_size;
        let block_offset = offset % self.block_size;

        // 读取包含目标数据的块
        let mut buf = alloc::vec![0u8; self.block_size];
        match self.inner.read_block(block_id, &mut buf) {
            Ok(_) => {
                // 返回从block_offset开始的数据
                // ext4_rs通常期望读取整个块，但从指定偏移开始
                if block_offset == 0 {
                    buf
                } else {
                    // 需要拼接两个块
                    let mut result = buf[block_offset..].to_vec();
                    // 尝试读取下一个块补齐
                    let mut next_buf = alloc::vec![0u8; self.block_size];
                    if self.inner.read_block(block_id + 1, &mut next_buf).is_ok() {
                        result.extend_from_slice(&next_buf[..block_offset]);
                    }
                    result
                }
            }
            Err(e) => {
                pr_err!("[Ext4Adapter] Read error at offset {}: {:?}", offset, e);
                // ext4_rs 的 read_offset 不返回 Result，只能返回空数据
                alloc::vec![0u8; self.block_size]
            }
        }
    }

    fn write_offset(&self, offset: usize, data: &[u8]) {
        let block_id = offset / self.block_size;

        // ext4_rs 通常写入完整块
        if data.len() == self.block_size {
            if let Err(e) = self.inner.write_block(block_id, data) {
                pr_err!("[Ext4Adapter] Write error at offset {}: {:?}", offset, e);
            }
        } else {
            // 处理非对齐写入：读-改-写
            let mut buf = alloc::vec![0u8; self.block_size];
            if self.inner.read_block(block_id, &mut buf).is_ok() {
                let block_offset = offset % self.block_size;
                let copy_len = data.len().min(self.block_size - block_offset);
                buf[block_offset..block_offset + copy_len].copy_from_slice(&data[..copy_len]);

                if let Err(e) = self.inner.write_block(block_id, &buf) {
                    pr_err!("[Ext4Adapter] Write error at offset {}: {:?}", offset, e);
                }
            }
        }
    }
}
