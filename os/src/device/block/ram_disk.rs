use super::super::{DeviceType, Driver};
use super::BlockDriver;
use crate::sync::SpinLock;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

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

    /// 获取设备 ID
    pub fn device_id(&self) -> usize {
        self.device_id
    }
}

impl Driver for RamDisk {
    fn try_handle_interrupt(&self, _irq: Option<usize>) -> bool {
        false // RamDisk 不处理中断
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Block
    }

    fn get_id(&self) -> String {
        alloc::format!("ramdisk_{}", self.device_id)
    }

    fn as_block(&self) -> Option<&dyn BlockDriver> {
        Some(self)
    }
}

// 实现 BlockDriver trait
impl BlockDriver for RamDisk {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> bool {
        if buf.len() != self.block_size {
            return false;
        }

        let data = self.data.lock();
        let offset = block_id * self.block_size;

        if offset + self.block_size > data.len() {
            return false;
        }

        buf.copy_from_slice(&data[offset..offset + self.block_size]);
        true
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) -> bool {
        if buf.len() != self.block_size {
            return false;
        }

        let mut data = self.data.lock();
        let offset = block_id * self.block_size;

        if offset + self.block_size > data.len() {
            return false;
        }

        data[offset..offset + self.block_size].copy_from_slice(buf);
        true
    }

    fn flush(&self) -> bool {
        true // 内存设备无需 flush
    }

    fn block_size(&self) -> usize {
        self.block_size
    }

    fn total_blocks(&self) -> usize {
        self.data.lock().len() / self.block_size
    }
}
