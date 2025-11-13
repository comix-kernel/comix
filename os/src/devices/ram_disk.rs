use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use crate::sync::SpinLock;
use super::block_device::{BlockDevice, BlockError};

/// 内存模拟的块设备
///
/// 用于测试和开发
pub struct RamDisk {
    /// 存储数据
    data: SpinLock<Vec<u8>>,

    /// 块大小
    block_size: usize,

    /// 设备 ID
    device_id: usize,
}

impl RamDisk {
    /// 创建指定大小的内存磁盘
    pub fn new(size: usize, block_size: usize, device_id: usize) -> Arc<Self> {
        Arc::new(Self {
            data: SpinLock::new(vec![0u8; size]),
            block_size,
            device_id,
        })
    }

    /// 从字节数组创建
    pub fn from_bytes(data: Vec<u8>, block_size: usize, device_id: usize) -> Arc<Self> {
        Arc::new(Self {
            data: SpinLock::new(data),
            block_size,
            device_id,
        })
    }

    /// 获取原始数据（用于调试）
    pub fn raw_data(&self) -> Vec<u8> {
        self.data.lock().clone()
    }
}

impl BlockDevice for RamDisk {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> Result<(), BlockError> {
        if buf.len() != self.block_size {
            return Err(BlockError::InvalidBlock);
        }

        let data = self.data.lock();
        let offset = block_id * self.block_size;

        if offset + self.block_size > data.len() {
            return Err(BlockError::InvalidBlock);
        }

        buf.copy_from_slice(&data[offset..offset + self.block_size]);
        Ok(())
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) -> Result<(), BlockError> {
        if buf.len() != self.block_size {
            return Err(BlockError::InvalidBlock);
        }

        let mut data = self.data.lock();
        let offset = block_id * self.block_size;

        if offset + self.block_size > data.len() {
            return Err(BlockError::InvalidBlock);
        }

        data[offset..offset + self.block_size].copy_from_slice(buf);
        Ok(())
    }

    fn block_size(&self) -> usize {
        self.block_size
    }

    fn total_blocks(&self) -> usize {
        self.data.lock().len() / self.block_size
    }

    fn flush(&self) -> Result<(), BlockError> {
        // 内存设备无需 flush
        Ok(())
    }

    fn device_id(&self) -> usize {
        self.device_id
    }
}
